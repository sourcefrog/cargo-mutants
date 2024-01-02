// Copyright 2023 Martin Pool

//! Filter mutants to those intersecting a diff on the file tree,
//! for example from uncommitted or unmerged changes.

use std::collections::HashMap;

use anyhow::{anyhow, bail};
use camino::Utf8Path;
use indoc::formatdoc;
use itertools::Itertools;
use patch::{Line, Patch};
use tracing::{trace, warn};

use crate::mutate::Mutant;
use crate::source::SourceFile;
use crate::Result;

/// Return only mutants to functions whose source was touched by this diff.
pub fn diff_filter(mutants: Vec<Mutant>, diff_text: &str) -> Result<Vec<Mutant>> {
    // Flatten the error to a string because otherwise it references the diff, and can't be returned.
    let patches =
        Patch::from_multiple(diff_text).map_err(|err| anyhow!("Failed to parse diff: {err}"))?;
    check_diff_new_text_matches(&patches, &mutants)?;
    let mut lines_changed_by_path: HashMap<&Utf8Path, Vec<usize>> = HashMap::new();
    for patch in &patches {
        let path = strip_patch_path(&patch.new.path);
        if path == "/dev/null" {
            // The file was deleted; we can't possibly match anything in it.
            continue;
        }
        if lines_changed_by_path
            .insert(path, affected_lines(patch))
            .is_some()
        {
            bail!("Patch input contains repeated filename: {path:?}");
        }
    }
    let mut matched: Vec<Mutant> = Vec::with_capacity(mutants.len());
    'mutant: for mutant in mutants.into_iter() {
        let path = mutant.source_file.path();
        if let Some(lines_changed) = lines_changed_by_path.get(path) {
            // We could do be smarter about searching for an intersection of ranges, rather
            // than probing one line at a time... But, the numbers are likely to be small
            // enough that this is tolerable...
            //
            // We could also search for each unique span in each file, and then include
            // every mutant that intersects any of those spans, since commonly there will
            // be multiple mutants in the same function.
            for line in mutant.span.start.line..=mutant.span.end.line {
                if lines_changed.binary_search(&line).is_ok() {
                    trace!(
                        ?path,
                        line,
                        mutant = mutant.name(true, false),
                        "diff matched mutant"
                    );
                    matched.push(mutant);
                    continue 'mutant;
                }
            }
        }
    }
    Ok(matched)
}

/// Error if the new text from the diffs doesn't match the source files.
fn check_diff_new_text_matches(patches: &[Patch], mutants: &[Mutant]) -> Result<()> {
    let mut source_by_name: HashMap<&Utf8Path, &SourceFile> = HashMap::new();
    for mutant in mutants {
        source_by_name
            .entry(mutant.source_file.path())
            .or_insert_with(|| &mutant.source_file);
    }
    for patch in patches {
        let path = strip_patch_path(&patch.new.path);
        if let Some(source_file) = source_by_name.get(&path) {
            let reconstructed = partial_new_file(patch);
            let lines = source_file.code().lines().collect_vec();
            for (lineno, diff_content) in reconstructed {
                let source_content = lines.get(lineno - 1).unwrap_or(&"");
                if diff_content != *source_content {
                    warn!(
                        ?path,
                        lineno,
                        ?diff_content,
                        ?source_content,
                        "Diff content doesn't match source file"
                    );
                    bail!(formatdoc! { "\
                        Diff content doesn't match source file: {path} line {lineno}
                        diff has:   {diff_content:?}
                        source has: {source_content:?}
                        The diff might be out of date with this source tree.
                    "});
                }
            }
        }
    }
    Ok(())
}

/// Remove the `b/` prefix commonly found in paths within diffs.
fn strip_patch_path(path: &str) -> &Utf8Path {
    let path = Utf8Path::new(path);
    path.strip_prefix("b").unwrap_or(path)
}

/// Given a diff, return the ranges of actually-changed lines, ignoring context lines.
///
/// Code that's only included as context doesn't need to be tested.
///
/// This returns a list of line numbers that are either added to the new file, or
/// adjacent to deletions.
///
/// (A list of ranges would be more concise but this is easier for a first version.)
///
/// If a line is deleted then the range will span from the line before to the line after.
fn affected_lines(patch: &Patch) -> Vec<usize> {
    let mut r = Vec::new();
    for hunk in &patch.hunks {
        let mut lineno: usize = hunk.new_range.start.try_into().unwrap();
        // True if the previous line was deleted. If set, then the next line that exists in the
        // new file, if there is one, will be marked as affected.
        let mut prev_removed = false;
        for line in &hunk.lines {
            match line {
                Line::Remove(_) => {
                    prev_removed = true;
                }
                Line::Add(_) | Line::Context(_) => {
                    if prev_removed {
                        debug_assert!(
                            r.last().map_or(true, |last| *last < lineno),
                            "{lineno} {r:?}"
                        );
                        debug_assert!(lineno >= 1, "{lineno}");
                        r.push(lineno);
                        prev_removed = false;
                    }
                }
            }
            match line {
                Line::Context(_) => {
                    lineno += 1;
                }
                Line::Add(_) => {
                    if r.last().map_or(true, |last| *last < lineno) {
                        r.push(lineno);
                    }
                    lineno += 1;
                }
                Line::Remove(_) => {
                    if lineno > 1 && r.last().map_or(true, |last| *last < (lineno - 1)) {
                        r.push(lineno - 1);
                    }
                }
            }
        }
    }
    debug_assert!(
        r.iter().tuple_windows().all(|(a, b)| a < b),
        "remove_context: line numbers not sorted and unique: {r:?}"
    );
    r
}

/// Recreate a partial view of the new file from a Patch.
///
/// This contains lines present as adedd or context. Typically not all context
/// will be covered, so the output is a list of line numbers and their text.
fn partial_new_file<'d>(patch: &Patch<'d>) -> Vec<(usize, &'d str)> {
    let mut r: Vec<(usize, &'d str)> = Vec::new();
    for hunk in &patch.hunks {
        let mut lineno: usize = hunk.new_range.start.try_into().unwrap();
        for line in &hunk.lines {
            match line {
                Line::Context(text) | Line::Add(text) => {
                    debug_assert!(lineno >= 1, "{lineno}");
                    debug_assert!(
                        r.last().map_or(true, |last| last.0 < lineno),
                        "{lineno} {r:?}"
                    );
                    r.push((lineno, text));
                    lineno += 1;
                }
                Line::Remove(_) => {}
            }
        }
        debug_assert_eq!(
            lineno,
            (hunk.new_range.start + hunk.new_range.count) as usize,
            "Wrong number of resulting lines?"
        );
    }
    r
}

#[cfg(test)]
mod test_super {
    use std::fs::read_to_string;

    use pretty_assertions::assert_eq;
    use similar::TextDiff;

    use super::*;

    #[test]
    fn patch_parse_error() {
        let diff = "not really a diff\n";
        let err = diff_filter(Vec::new(), diff).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Failed to parse diff: Line 1: Error while parsing: not really a diff\n"
        );
    }

    #[test]
    fn read_diff_with_empty_mutants() {
        let diff = "\
diff --git a/src/mutate.rs b/src/mutate.rs
index eb42779..a0091b7 100644
--- a/src/mutate.rs
+++ b/src/mutate.rs
@@ -6,9 +6,7 @@ use std::fmt;
 use std::fs;
 use std::sync::Arc;
 use std::foo;
-use anyhow::ensure;
-use anyhow::Context;
-use anyhow::Result;
+use anyhow::{ensure, Context, Result};
 use serde::ser::{SerializeStruct, Serializer};
 use serde::Serialize;
 use similar::TextDiff;
";
        let filtered: Vec<Mutant> = diff_filter(Vec::new(), diff).expect("diff filtered");
        assert_eq!(filtered.len(), 0);
    }

    fn make_diff(old: &str, new: &str) -> String {
        TextDiff::from_lines(old, new)
            .unified_diff()
            .context_radius(2)
            .header("a/file.rs", "b/file.rs")
            .to_string()
    }

    #[test]
    fn strip_patch_path_prefix() {
        assert_eq!(strip_patch_path("b/src/mutate.rs"), "src/mutate.rs");
    }

    #[test]
    fn affected_lines_from_single_insertion() {
        let orig_lines = (1..=4).map(|i| format!("line {}\n", i)).collect_vec();
        for i in 1..=5 {
            let mut new = orig_lines.clone();
            let new_value = "new line\n".to_owned();
            if i < 5 {
                new.insert(i - 1, new_value);
            } else {
                new.push(new_value);
            }
            let diff = make_diff(&orig_lines.join(""), &new.join(""));
            println!("{diff}");
            let patch = Patch::from_single(&diff).unwrap();
            let affected = affected_lines(&patch);
            // When we insert a line then only that one line is affected.
            assert_eq!(affected, &[i]);
        }
    }

    #[test]
    fn affected_lines_from_single_deletion() {
        let orig_lines = (1..=5).map(|i| format!("line {}\n", i)).collect_vec();
        for i in 1..=5 {
            let mut new = orig_lines.clone();
            new.remove(i - 1);
            let diff = make_diff(&orig_lines.join(""), &new.join(""));
            println!("{diff}");
            let patch = Patch::from_single(&diff).unwrap();
            let affected = affected_lines(&patch);
            // If line 1 is removed we should see line 1 as affected. If line 2 is removed
            // then 1 and 2 are affected, etc. If line 5 is removed, then only 4, the last
            // remaining line is affected.
            match i {
                1 => assert_eq!(affected, &[1]),
                5 => assert_eq!(affected, &[4]),
                i => assert_eq!(affected, &[i - 1, i]),
            }
        }
    }

    #[test]
    fn affected_lines_from_double_deletion() {
        let orig_lines = (1..=5).map(|i| format!("line {}\n", i)).collect_vec();
        for i in 1..=4 {
            let mut new = orig_lines.clone();
            new.remove(i - 1);
            new.remove(i - 1);
            let diff = make_diff(&orig_lines.join(""), &new.join(""));
            println!("{diff}");
            let patch = Patch::from_single(&diff).unwrap();
            let affected = affected_lines(&patch);
            match i {
                1 => assert_eq!(affected, &[1]),
                4 => assert_eq!(affected, &[3]),
                2 | 3 => assert_eq!(affected, &[i - 1, i]),
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn affected_lines_from_replacement() {
        let orig_lines = (1..=5).map(|i| format!("line {}\n", i)).collect_vec();
        for i in 1..=5 {
            let insertion = ["new 1\n".to_owned(), "new 2\n".to_owned()];
            let new = orig_lines[..(i - 1)]
                .iter()
                .cloned()
                .chain(insertion)
                .chain(orig_lines[i..].iter().cloned())
                .collect_vec();
            let diff = make_diff(&orig_lines.join(""), &new.join(""));
            println!("{diff}");
            let patch = Patch::from_single(&diff).unwrap();
            let affected = affected_lines(&patch);
            if i > 1 {
                // The line before the deletion also counts as affected.
                assert_eq!(affected, &[i - 1, i, i + 1]);
            } else {
                assert_eq!(affected, &[i, i + 1]);
            }
        }
    }

    #[test]
    fn reconstruct_partial_new_file() {
        let old = read_to_string("testdata/diff0/src/lib.rs").unwrap();
        let new = read_to_string("testdata/diff1/src/lib.rs").unwrap();
        let diff = make_diff(&old, &new);
        let patch = Patch::from_single(&diff).unwrap();
        let reconstructed = partial_new_file(&patch);
        println!("{reconstructed:#?}");
        assert_eq!(reconstructed.len(), 16);
        let new_lines = new.lines().collect_vec();
        for (lineno, text) in reconstructed {
            assert_eq!(text, new_lines[lineno - 1]);
        }
    }
}

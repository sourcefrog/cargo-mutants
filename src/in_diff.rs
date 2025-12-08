// Copyright 2023 - 2025 Martin Pool

//! Filter mutants to those intersecting a diff on the file tree,
//! for example from uncommitted or unmerged changes.

use std::collections::HashMap;
use std::fmt::Display;
use std::fs::File;
use std::io::Read;

use anyhow::bail;
use camino::Utf8Path;
use gitpatch::{Line, Patch};
use indoc::formatdoc;
use itertools::Itertools;
use tracing::{error, trace, warn};

use crate::mutant::Mutant;
use crate::source::SourceFile;
use crate::{exit_code, Result};

/// The result of filtering mutants based on a diff.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum DiffFilterError {
    /// The diff is empty.
    EmptyDiff,
    /// The diff new text doesn't match the source tree.
    MismatchedDiff(String),
    /// The diff is not empty but doesn't intersect any mutants.
    NoMutants,
    /// The diff is not empty but changes no Rust source files.
    NoSourceFiles,
    /// The diff can't be parsed.
    InvalidDiff(String),
    /// Can't open or read the diff file.
    File(String),
}

impl DiffFilterError {
    /// Return the overall exit code for the error.
    ///
    /// Some errors such as an empty diff or one that changes no Rust source files
    /// still mean we can't process any mutants, but it's not necessarily a problem.
    pub fn exit_code(&self) -> i32 {
        match self {
            DiffFilterError::EmptyDiff
            | DiffFilterError::NoSourceFiles
            | DiffFilterError::NoMutants => exit_code::SUCCESS,
            DiffFilterError::MismatchedDiff(_) => exit_code::FILTER_DIFF_MISMATCH,
            DiffFilterError::File(_) | DiffFilterError::InvalidDiff(_) => {
                exit_code::FILTER_DIFF_INVALID
            }
        }
    }
}

impl Display for DiffFilterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiffFilterError::EmptyDiff => write!(f, "Diff file is empty"),
            DiffFilterError::NoSourceFiles => write!(f, "Diff changes no Rust source files"),
            DiffFilterError::NoMutants => write!(f, "No mutants to filter"),
            DiffFilterError::MismatchedDiff(msg) => write!(f, "{msg}"),
            DiffFilterError::InvalidDiff(msg) => write!(f, "Failed to parse diff: {msg}"),
            DiffFilterError::File(msg) => write!(f, "Failed to read diff file: {msg}"),
        }
    }
}

pub fn diff_filter_file(
    mutants: Vec<Mutant>,
    diff_path: &Utf8Path,
) -> Result<Vec<Mutant>, DiffFilterError> {
    let mut diff_file = File::open(diff_path).map_err(|err| {
        error!("Failed to open diff file: {err}");
        DiffFilterError::File(err.to_string())
    })?;
    let mut diff_bytes = Vec::new();
    diff_file.read_to_end(&mut diff_bytes).map_err(|err| {
        error!("Failed to read diff file: {err}");
        DiffFilterError::File(err.to_string())
    })?;
    // The diff might contain non-UT8 in files that aren't part of Rust source files.
    // Rust must be UTF-8 so we can ignore any decoding errors.
    let diff_text = String::from_utf8_lossy(&diff_bytes);
    diff_filter(mutants, &diff_text)
}

/// Filter a list of mutants to those intersecting a diff on the file tree.
pub fn diff_filter(mutants: Vec<Mutant>, diff_text: &str) -> Result<Vec<Mutant>, DiffFilterError> {
    if diff_text.trim().is_empty() {
        return Err(DiffFilterError::EmptyDiff);
    }
    let fixed_diff = diff_text;
    // Our diff library treats an empty diff as an error, which perhaps is not the correct behavior.
    // If after stripping binaries it's empty, let's say it matched nothing, rather than
    // being strictly empty. This would be fixed by <https://github.com/gitpatch-rs/gitpatch/pull/14>.
    if fixed_diff.trim().is_empty() {
        return Err(DiffFilterError::NoSourceFiles);
    }
    let patches = match Patch::from_multiple(&fixed_diff) {
        Ok(patches) => patches,
        Err(err) => return Err(DiffFilterError::InvalidDiff(err.to_string())), // squash to a string to simplify lifetimes
    };
    if let Err(err) = check_diff_new_text_matches(&patches, &mutants) {
        return Err(DiffFilterError::MismatchedDiff(err.to_string()));
    }
    let mut lines_changed_by_path: HashMap<&Utf8Path, Vec<usize>> = HashMap::new();
    let mut changed_rs_file = false;
    for patch in &patches {
        let path = strip_patch_path(&patch.new.path);
        if path != "/dev/null" && path.extension() == Some("rs") {
            changed_rs_file = true;
            lines_changed_by_path
                .entry(path)
                .or_default()
                .extend(affected_lines(patch));
        }
    }
    let mut matched: Vec<Mutant> = Vec::with_capacity(mutants.len());
    'mutant: for mutant in mutants {
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
                        mutant = mutant.name(true),
                        "diff matched mutant"
                    );
                    matched.push(mutant);
                    continue 'mutant;
                }
            }
        }
    }
    if matched.is_empty() {
        if changed_rs_file {
            trace!("diff matched no mutants");
            Err(DiffFilterError::NoMutants)
        } else {
            Err(DiffFilterError::NoSourceFiles)
        }
    } else {
        Ok(matched)
    }
}

/// Check that the "new" side of the text matches the source in this tree.
///
/// This is a convenience function to make sure the user supplied a valid diff: if not,
/// the regions in the diff will be meaningless.
///
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
    let mut affected_lines = Vec::new();
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
                            affected_lines.last().map_or(true, |last| *last < lineno),
                            "{lineno} {affected_lines:?}"
                        );
                        debug_assert!(lineno >= 1, "{lineno}");
                        affected_lines.push(lineno);
                        prev_removed = false;
                    }
                }
            }
            match line {
                Line::Context(_) => {
                    lineno += 1;
                }
                Line::Add(_) => {
                    if affected_lines.last().map_or(true, |last| *last != lineno) {
                        affected_lines.push(lineno);
                    }
                    lineno += 1;
                }
                Line::Remove(_) => {
                    if lineno > 1
                        && affected_lines
                            .last()
                            .map_or(true, |last| *last != (lineno - 1))
                    {
                        affected_lines.push(lineno - 1);
                    }
                }
            }
        }
    }
    debug_assert!(
        affected_lines.iter().tuple_windows().all(|(a, b)| a < b),
        "remove_context: line numbers not sorted and unique: {affected_lines:?}"
    );
    affected_lines
}

/// Recreate a partial view of the new file from a Patch.
///
/// A patch can contain lines that are added, deleted, or unchanged context.
///
/// By extracting the added lines and context lines, we can recreate a partial
/// view of the new file. From this we can check that the patch matches the
/// expected changes.
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
            Ok(lineno),
            (hunk.new_range.start + hunk.new_range.count).try_into(),
            "Wrong number of resulting lines?"
        );
    }
    r
}

#[cfg(test)]
mod test_super {
    use std::fs::read_to_string;

    use indoc::indoc;
    use pretty_assertions::assert_eq;
    use similar::TextDiff;

    use super::*;

    #[test]
    fn patch_parse_error() {
        let diff = "not really a diff\n";
        let err = diff_filter(Vec::new(), diff).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Failed to parse diff: Line 1: Error while parsing: \"not really a diff\\n\": Eof"
        );
    }

    #[test]
    fn parse_git_binary_diff() {
        // From <https://github.com/sourcefrog/cargo-mutants/issues/391>
        let diff = indoc! {
            "diff --git a/test-renderers/expected/renderers/fog-None-wgpu.png b/test-renderers/expected/renderers/fog-None-wgpu.png
            index 616d6ea8..afd1b043 100644
            Binary files a/test-renderers/expected/renderers/fog-None-wgpu.png and b/test-renderers/expected/renderers/fog-None-wgpu.png differ
            "
        };
        assert_eq!(
            diff_filter(Vec::new(), diff),
            Err(DiffFilterError::NoSourceFiles)
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
        let err = diff_filter(Vec::new(), diff);
        assert_eq!(err, Err(DiffFilterError::NoMutants));
        assert_eq!(err.unwrap_err().exit_code(), 0);
    }

    #[test]
    fn read_diff_with_no_sourcecode() {
        let diff = "\
diff --git a/book/src/baseline.md b/book/src/baseline.md
index cc3ce8c..8fe9aa0 100644
--- a/book/src/baseline.md
+++ b/book/src/baseline.md
@@ -1,6 +1,6 @@
    # Baseline tests
-Normally, cargo-mutants builds
+Normally cargo-mutants builds
";
        let err = diff_filter(Vec::new(), diff);
        assert_eq!(err, Err(DiffFilterError::NoSourceFiles));
        assert_eq!(err.unwrap_err().exit_code(), 0);
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
        let orig_lines = (1..=4).map(|i| format!("line {i}\n")).collect_vec();
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
        let orig_lines = (1..=5).map(|i| format!("line {i}\n")).collect_vec();
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
        let orig_lines = (1..=5).map(|i| format!("line {i}\n")).collect_vec();
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
        let orig_lines = (1..=5).map(|i| format!("line {i}\n")).collect_vec();
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

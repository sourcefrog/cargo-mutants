// Copyright 2023 Martin Pool

//! Filter mutants to those intersecting a diff on the file tree,
//! for example from uncommitted or unmerged changes.

#![allow(unused_imports)]

use std::collections::HashMap;

use anyhow::{anyhow, bail, Context};
use camino::Utf8Path;
use itertools::Itertools;
use patch::{Line, Patch, Range};
use tracing::{trace, warn};

use crate::mutate::Mutant;
use crate::Result;

/// Return only mutants to functions whose source was touched by this diff.
pub fn diff_filter(mutants: Vec<Mutant>, diff_text: &str) -> Result<Vec<Mutant>> {
    // Flatten the error to a string because otherwise it references the diff, and can't be returned.
    let patches =
        Patch::from_multiple(diff_text).map_err(|err| anyhow!("Failed to parse diff: {err}"))?;
    let mut lines_changed_by_path: HashMap<&Utf8Path, Vec<usize>> = HashMap::new();
    for patch in &patches {
        let path = strip_patch_path(&patch.new.path);
        if lines_changed_by_path
            .insert(path, remove_context(patch))
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
                        %mutant,
                        mutant_span = ?&mutant.span,
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
fn remove_context(patch: &Patch) -> Vec<usize> {
    let mut r = Vec::new();
    for hunk in &patch.hunks {
        let mut lineno: usize = hunk.new_range.start.try_into().unwrap();
        for line in &hunk.lines {
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
                    if lineno > 0 && r.last().map_or(true, |last| *last < lineno) {
                        r.push(lineno);
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

#[cfg(test)]
mod test_super {
    use pretty_assertions::assert_eq;

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
}

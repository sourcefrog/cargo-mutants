// Copyright 2023 Martin Pool

//! Filter mutants to those intersecting a diff on the file tree,
//! for example from uncommitted or unmerged changes.

#![allow(unused_imports)]

use std::collections::HashMap;

use anyhow::{anyhow, bail, Context};
use camino::Utf8Path;
use patch::{Patch, Range};
use tracing::warn;

use crate::mutate::Mutant;
use crate::Result;

/// Return only mutants to functions whose source was touched by this diff.
pub fn diff_filter<'a>(mutants: &[&'a Mutant], diff_text: &str) -> Result<Vec<&'a Mutant>> {
    // Flatten the error to a string because otherwise it references the diff, and can't be returned.
    let patches =
        Patch::from_multiple(diff_text).map_err(|err| anyhow!("Failed to parse diff: {err}"))?;
    let mut patch_by_path: HashMap<&Utf8Path, &Patch> = HashMap::new();
    for patch in &patches {
        let path = strip_patch_path(&patch.new.path);
        if patch_by_path.insert(path, patch).is_some() {
            bail!("Patch input contains repeated filename: {path:?}");
        }
    }

    /* TODO: Find the intersection of the patches and mutants:

    For each patch, changing one file:

    Only mutants matching that file could be relevant: we might need some heuristics to
    strip a `b/` prefix off the filesname.

    The naive way is quadratic but we could first group the mutants by filename. And,
    there are probably not so many mutants to make it too expensive for a first version.

    Allow for diffs that might have multiple changes to the same file.
    We shouldn't duplicate mutants even if the diffs have duplicates.
    */
    let mut matched: Vec<&Mutant> = Vec::with_capacity(mutants.len());
    'mutant: for mutant in mutants {
        if let Some(patch) = patch_by_path.get(mutant.source_file.path()) {
            for hunk in &patch.hunks {
                if range_overlaps(&hunk.new_range, mutant) {
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

fn range_overlaps(diff_range: &Range, mutant: &Mutant) -> bool {
    let diff_end = diff_range.start + diff_range.count;
    diff_end >= mutant.span.start.line.try_into().unwrap()
        && diff_range.start <= mutant.span.end.line.try_into().unwrap()
}

#[cfg(test)]
mod test_super {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn patch_parse_error() {
        let diff = "not really a diff\n";
        let err = diff_filter(&[], diff).unwrap_err();
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
        let filtered: Vec<&Mutant> = diff_filter(&[], diff).expect("diff filtered");
        assert_eq!(filtered.len(), 0);
    }
}

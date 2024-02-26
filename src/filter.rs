// Copyright 2024 Martin Pool

//! Filter and exclude mutants.

use std::collections::{HashMap, HashSet};
use std::fs::read_to_string;

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use once_cell::sync::Lazy;
// use lazy_static::lazy_static;
use regex::Regex;
use tracing::{debug, trace, warn};

use crate::mutate::Mutant;
use crate::Result;

/* When filtering by name, we match the filename and the function name, and the description
 * of the mutant ("replace good by bad"), but not the line/column because they might easily
 * change as the tree is edited.
 *
 * First, pull all the names into a filter struct, that groups them by filename and function,
 * and then within that a list of descriptions.
 *
 * The description (without line/col) might not be unique within a function.
 *
 * This could match a bit more efficiently if the discovered mutants were kept in lists per
 * file; maybe later.
 */

/// A filter that can match mutants from a list, matching on filename and
/// description (possibly including the function name), and ignoring line/column.
///
/// The filter can be applied as either an include or exclude filter.
#[derive(Debug, Clone, Default)]
pub struct NameFilter {
    /// Map from (path, function) to a list of descriptions.
    by_file: HashMap<Utf8PathBuf, HashSet<String>>,
}

impl NameFilter {
    /// Build a NameFilter from the lines in the given files.
    ///
    /// Returns Ok(None) if no files are given.
    pub fn from_files(
        filenames: impl IntoIterator<Item = impl AsRef<Utf8Path>>,
    ) -> Result<Option<Self>> {
        let filenames = filenames.into_iter().collect::<Vec<_>>();
        if filenames.is_empty() {
            return Ok(None);
        }
        Ok(Some(
            filenames
                .into_iter()
                .map(|filename| {
                    let filename = filename.as_ref();
                    read_to_string(filename).with_context(|| "Read filter file {filename:?}")
                })
                .collect::<Result<Vec<String>>>()?
                .into_iter()
                .fold(NameFilter::default(), |mut filter, content| {
                    // Not quite using FromIterator here because of annoying errors that you
                    // can't return a reference to each line from a closure, therefore I can't
                    // easily build an iterator over the lines in all the files...
                    for line in content.lines() {
                        filter.add_line(line);
                    }
                    filter
                }),
        ))
    }

    pub fn matches(&self, mutant: &Mutant) -> bool {
        // Maybe this clones too much here; maybe it's not a big deal.
        let path = mutant.source_file.path();
        let description = mutant.describe_change();
        debug!(?path, ?description, "match mutant");
        self.by_file
            .get(path)
            .map(|descriptions| descriptions.contains(&description))
            .unwrap_or(false)
    }

    fn add_line(&mut self, line: &str) {
        if let Some((path, description)) = parse_line(line) {
            self.by_file.entry(path).or_default().insert(description);
        }
    }
}

impl<S> FromIterator<S> for NameFilter
where
    S: AsRef<str>,
{
    fn from_iter<I: IntoIterator<Item = S>>(iter: I) -> Self {
        iter.into_iter()
            .fold(NameFilter::default(), |mut filter, line| {
                filter.add_line(line.as_ref());
                filter
            })
    }
}

/// Parse a line into a filter entry.
///
/// The line is like: `src/lib.rs:102:1: foo replace good by bad in some_function`,
/// or the line, line&col can be omitted.
///
/// Returns None and emits a warning if the line can't be parsed.
fn parse_line(line: &str) -> Option<(Utf8PathBuf, String)> {
    static LINE_RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r#"^([^:]+)(?::\d+)?(?::\d+)?: (.+)$"#).unwrap());
    if let Some(captures) = LINE_RE.captures(line) {
        trace!(?captures, ?line, "parse name filter line");
        let path: Utf8PathBuf = captures.get(1)?.as_str().into();
        let description = captures.get(2)?.as_str().to_string();
        Some((path, description))
    } else {
        warn!(?line, "Can't parse line as \"FILE:LINE:COL: DESCRIPTION\"");
        None
    }
}

#[cfg(test)]
mod test {
    use super::parse_line;

    #[test]
    fn parse_line_without_line_col_or_function() {
        let line = "src/lib.rs: foo replace good by bad";
        assert_eq!(
            parse_line(line),
            Some(("src/lib.rs".into(), "foo replace good by bad".into()))
        );
    }

    #[test]
    fn parse_line_with_line_col_without_function() {
        let line = "src/lib.rs:123:45: foo replace good by bad";
        assert_eq!(
            parse_line(line),
            Some(("src/lib.rs".into(), "foo replace good by bad".into()))
        );
    }

    #[test]
    fn parse_line_without_line_col_with_function() {
        let line = "src/lib.rs: foo replace good by bad in some_function";
        assert_eq!(
            parse_line(line),
            Some((
                "src/lib.rs".into(),
                "foo replace good by bad in some_function".into()
            ))
        );
    }

    #[test]
    fn parse_line_with_line_col_and_function() {
        let line = "src/lib.rs:102:1: foo replace good by bad in some_function";
        assert_eq!(
            parse_line(line),
            Some((
                "src/lib.rs".into(),
                "foo replace good by bad in some_function".into()
            ))
        );
    }

    #[test]
    fn parse_line_with_function_trait_type() {
        let line = "src/visit.rs:289:36: replace || with && in <impl Visit for DiscoveryVisitor<'_>>::visit_trait_item_fn";
        assert_eq!(
            parse_line(line),
            Some((
                "src/visit.rs".into(),
                "replace || with && in <impl Visit for DiscoveryVisitor<'_>>::visit_trait_item_fn"
                    .into()
            ))
        );
    }
}

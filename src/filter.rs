// Copyright 2024 Martin Pool

//! Filter and exclude mutants.

use std::collections::{HashMap, HashSet};

use camino::Utf8PathBuf;
use once_cell::sync::Lazy;
// use lazy_static::lazy_static;
use regex::Regex;
use tracing::{trace, warn};

use crate::mutate::Mutant;

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

/// A filter that can match mutants from a list, matching on filename, function name, and
/// description and ignoring line/column.
///
/// The filter can be applied as either an include or exclude filter.
#[derive(Debug, Default)]
struct NameFilter {
    /// Map from (path, function) to a list of descriptions.
    by_file: HashMap<(Utf8PathBuf, Option<String>), HashSet<String>>,
}

impl NameFilter {
    pub fn matches(&self, mutant: &Mutant) -> bool {
        // Maybe this clones too much here; maybe it's not a big deal.
        self.by_file
            .get(&(
                mutant.source_file.path().into(),
                mutant.function.as_ref().map(|f| f.function_name.clone()),
            ))
            .map(|descriptions| descriptions.contains(&mutant.describe_change()))
            .unwrap_or(false)
    }
}

impl<S> FromIterator<S> for NameFilter
where
    S: AsRef<str>,
{
    fn from_iter<I: IntoIterator<Item = S>>(iter: I) -> Self {
        let mut filter = NameFilter::default();
        for line in iter.into_iter() {
            if let Some((path, function, description)) = parse_line(line.as_ref()) {
                filter
                    .by_file
                    .entry((path, function))
                    .or_default()
                    .insert(description);
            }
        }
        filter
    }
}

/// Parse a line into a filter entry.
///
/// The line is like: `src/lib.rs:102:1: foo replace good by bad in some_function`,
/// or the line, line&col, or function name can be omitted.
///
/// Returns None and emits a warning if the line can't be parsed.
fn parse_line(line: &str) -> Option<(Utf8PathBuf, Option<String>, String)> {
    static LINE_RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r#"^([^:]+)(?::\d+)?(?::\d+)?: (.+?)(?: in (.+))?$"#).unwrap());
    if let Some(captures) = LINE_RE.captures(line) {
        trace!(?captures, ?line, "parse name filter line");
        let path: Utf8PathBuf = captures.get(1)?.as_str().into();
        let description = captures.get(2)?.as_str().to_string();
        let function = captures.get(3).map(|m| m.as_str().to_string());
        Some((path, function, description))
    } else {
        warn!(
            ?line,
            "Can't parse line as \"FILE:LINE:COL: DESCRIPTION in FUNCTION\""
        );
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
            Some(("src/lib.rs".into(), None, "foo replace good by bad".into()))
        );
    }

    #[test]
    fn parse_line_with_line_col_without_function() {
        let line = "src/lib.rs:123:45: foo replace good by bad";
        assert_eq!(
            parse_line(line),
            Some(("src/lib.rs".into(), None, "foo replace good by bad".into()))
        );
    }

    #[test]
    fn parse_line_without_line_col_with_function() {
        let line = "src/lib.rs: foo replace good by bad in some_function";
        assert_eq!(
            parse_line(line),
            Some((
                "src/lib.rs".into(),
                Some("some_function".into()),
                "foo replace good by bad".into()
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
                Some("some_function".into()),
                "foo replace good by bad".into()
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
                Some("<impl Visit for DiscoveryVisitor<'_>>::visit_trait_item_fn".into()),
                "replace || with &&".into(),
            ))
        );
    }
}

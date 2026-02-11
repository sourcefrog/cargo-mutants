// Copyright 2024-2026 Martin Pool

//! Build globsets from lists of strings.

use std::borrow::Cow;

use anyhow::Context;
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

use crate::Result;

pub fn build_glob_set<S, I>(globs: I) -> Result<Option<GlobSet>>
where
    S: AsRef<str>,
    I: IntoIterator<Item = S>,
{
    let mut has_globs = false;
    let mut builder = GlobSetBuilder::new();
    for glob_str in globs {
        has_globs = true;
        let glob_str = glob_str.as_ref();
        let match_whole_path = if cfg!(windows) {
            glob_str.contains(['/', '\\'])
        } else {
            glob_str.contains('/')
        };
        let adjusted = if match_whole_path {
            vec![Cow::Borrowed(glob_str)]
        } else {
            vec![
                Cow::Owned(format!("**/{glob_str}")),
                Cow::Owned(format!("**/{glob_str}/**")),
            ]
        };
        for g in adjusted {
            builder.add(
                GlobBuilder::new(&g)
                    .literal_separator(true) // * does not match /
                    .build()
                    .with_context(|| format!("Failed to build glob from {glob_str:?}"))?,
            );
        }
    }
    if has_globs {
        Ok(Some(builder.build().context("Failed to build glob set")?))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn empty_globs() {
        assert!(
            build_glob_set(&[] as &[&str])
                .expect("build GlobSet")
                .is_none()
        );
    }

    #[test]
    fn literal_filename_matches_anywhere() {
        let set = build_glob_set(["foo.rs"])
            .expect("build GlobSet")
            .expect("GlobSet should not be empty");
        assert!(set.is_match("foo.rs"));
        assert!(set.is_match("src/foo.rs"));
        assert!(set.is_match("src/bar/foo.rs"));
        assert!(!set.is_match("src/bar/foo.rs~"));
        assert!(!set.is_match("src/bar/bar.rs"));
    }

    #[test]
    fn filename_matches_directories_and_their_contents() {
        let set = build_glob_set(["console"])
            .expect("build GlobSet")
            .expect("GlobSet should not be empty");
        assert!(set.is_match("console"));
        assert!(set.is_match("src/console"));
        assert!(set.is_match("src/bar/console"));
        assert!(set.is_match("src/bar/console/mod.rs"));
        assert!(set.is_match("src/console/ansi.rs"));
    }

    #[test]
    fn glob_without_slash_matches_filename_anywhere() {
        let set = build_glob_set(["*.rs"])
            .expect("build GlobSet")
            .expect("GlobSet should not be empty");
        assert!(set.is_match("foo.rs"));
        assert!(set.is_match("src/foo.rs"));
        assert!(set.is_match("src/bar/foo.rs"));
        assert!(!set.is_match("src/bar/foo.rs~"));
        assert!(set.is_match("src/bar/bar.rs"));
    }

    #[test]
    fn set_with_multiple_filenames() {
        let set = build_glob_set(["foo.rs", "bar.rs"])
            .expect("build GlobSet")
            .expect("GlobSet should not be empty");
        assert!(set.is_match("foo.rs"));
        assert!(set.is_match("bar.rs"));
        assert!(!set.is_match("baz.rs"));
    }

    #[test]
    fn glob_with_slashes_matches_whole_path() {
        let set = build_glob_set(["src/*.rs"])
            .expect("build GlobSet")
            .expect("GlobSet should not be empty");
        assert!(!set.is_match("foo.rs"));
        assert!(set.is_match("src/foo.rs"));
        assert!(!set.is_match("src/bar/foo.rs"));
        assert!(!set.is_match("src/foo.rs~"));
        assert!(
            !set.is_match("other/src/bar.rs"),
            "Glob with slashes anchors to whole path"
        );
    }

    #[test]
    fn starstar_at_start_of_path_matches_anywhere() {
        let set = build_glob_set(["**/foo.rs"])
            .expect("build GlobSet")
            .expect("GlobSet should not be empty");
        assert!(set.is_match("foo.rs"));
        assert!(set.is_match("src/foo.rs"));
        assert!(set.is_match("src/bar/foo.rs"));
        assert!(set.is_match("some/other/src/bar/foo.rs"));
        assert!(!set.is_match("src/bar/foo.rs~"));
        assert!(!set.is_match("src/bar/bar.rs"));
        assert!(!set.is_match("foo.rs/bar/bar.rs"));
    }

    #[test]
    fn starstar_within_path_matches_zero_or_more_directories() {
        let set = build_glob_set(["src/**/f*.rs"])
            .expect("build GlobSet")
            .expect("GlobSet should not be empty");
        assert!(!set.is_match("foo.rs"), "path must start with src");
        assert!(
            set.is_match("src/foo.rs"),
            "starstar can match zero directories"
        );
        assert!(set.is_match("src/bar/foo.rs"));
        assert!(set.is_match("src/bar/freq.rs"));
        assert!(
            !set.is_match("some/other/src/bar/foo.rs"),
            "path must start with src"
        );
        assert!(!set.is_match("src/bar/foo.rs~"));
        assert!(!set.is_match("src/bar/bar.rs"));
        assert!(!set.is_match("foo.rs/bar/bar.rs"));
    }

    #[test]
    #[cfg(unix)]
    fn on_unix_backslash_is_escape() {
        // weird glob but ok
        let set = build_glob_set(["src\\*.rs"])
            .expect("build GlobSet")
            .expect("GlobSet should not be empty");
        assert!(
            !set.is_match("src/foo.rs"),
            "backslash is not a path separator on Unix"
        );
        assert!(
            set.is_match("src*.rs"),
            "backslash escapes star (and is removed itself)"
        );
    }

    #[test]
    #[cfg(windows)]
    fn on_windows_backslash_is_path_separator() {
        let set = build_glob_set(&["src\\*.rs"])
            .expect("build GlobSet")
            .expect("GlobSet should not be empty");
        assert!(set.is_match("src\\foo.rs"));
        assert!(!set.is_match("src\\bar\\foo.rs"));
        assert!(!set.is_match("src\\foo.rs~"));
        assert!(
            !set.is_match("other\\src\\bar.rs"),
            "Glob with slashes anchors to whole path"
        );
    }
}

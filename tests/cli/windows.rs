// Copyright 2021-2023 Martin Pool

//! Windows-only CLI tests.

use super::{run, CommandInstaExt};

#[test]
fn list_mutants_well_tested_exclude_folder_containing_backslash_on_windows() {
    run()
        .arg("mutants")
        .args(["--list", "--exclude", "*\\module\\*"])
        .current_dir("testdata/with_child_directories")
        .assert_insta("list_mutants_well_tested_exclude_folder_filter");
}

// Copyright 2021-2023 Martin Pool

#![cfg(windows)]

//! Windows-only CLI tests.

use predicates::prelude::*;

mod util;
use util::run;

/// Only on Windows, backslash can be used as a path separator in filters.
#[test]
fn list_mutants_well_tested_exclude_folder_containing_backslash_on_windows() {
    run()
        .arg("mutants")
        .args(["--list", "--exclude", "*\\module\\*"])
        .current_dir("testdata/with_child_directories")
        .assert()
        .stdout(
            predicates::str::contains(r"src/module")
                .not()
                .and(predicates::str::contains(r"src/methods.rs")),
        );
}

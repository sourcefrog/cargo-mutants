// Copyright 2021-2024 Martin Pool

#![cfg(windows)]

//! Windows-only CLI tests.

use predicates::prelude::*;

mod util;
use util::{copy_of_testdata, run};

/// Only on Windows, backslash can be used as a path separator in filters.
#[test]
fn list_mutants_well_tested_exclude_folder_containing_backslash_on_windows() {
    // This could be written more simply as `--exclude module` but we want to
    // test that backslash is accepted.
    let tmp = copy_of_testdata("with_child_directories");
    run()
        .arg("mutants")
        .args(["--list", "--exclude", "**\\module\\**\\*.rs"])
        .current_dir(tmp.path())
        .assert()
        .stdout(
            predicates::str::contains(r"src/module")
                .not()
                .and(predicates::str::contains(r"src/methods.rs")),
        );
}

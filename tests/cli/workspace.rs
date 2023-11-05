// Copyright 2023 Martin Pool

//! Tests for cargo workspaces with multiple packages.

use super::run;

#[test]
fn list_warns_about_unmatched_packages() {
    run()
        .args([
            "mutants",
            "--list",
            "-d",
            "testdata/tree/workspace",
            "-p",
            "notapackage",
        ])
        .assert()
        .stdout(predicates::str::contains(
            "package notapackage not found in source tree",
        ))
        .code(0);
}

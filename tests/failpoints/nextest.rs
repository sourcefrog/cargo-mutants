// Copyright 2024 Martin Pool

//! Test simulated nextest failures.

use predicates::prelude::*;

use crate::common::{copy_of_testdata, run};

#[test]
fn nextest_unexpected_error_code_reported() {
    let tempdir = copy_of_testdata("small_well_tested");

    run()
        .arg("mutants")
        .arg("-d")
        .arg(tempdir.path())
        .arg("--test-tool=nextest")
        .env("FAILPOINTS", "Process::run=1*off->return(3)")
        .assert()
        .stderr(
            predicates::str::contains(
                "nextest process exited with unexpected code (not TEST_RUN_FAILED) code=3",
            )
            .and(predicates::str::contains(
                "cargo test failed in an unmutated tree, so no mutants were tested",
            )),
        )
        .failure();
}

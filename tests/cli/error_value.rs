// Copyright 2023 Martin Pool

//! Tests for error value mutations, from `--error-value` etc.

use std::env;

use predicates::prelude::*;

use super::{copy_of_testdata, run};

#[test]
fn error_value_catches_untested_ok_case() {
    // By default this tree should fail because it's configured to
    // generate an error value, and the tests forgot to check that
    // the code under test does return Ok.
    let tmp_src_dir = copy_of_testdata("error_value");
    run()
        .arg("mutants")
        .args(["-v", "-V", "--no-times", "--no-shuffle"])
        .arg("-d")
        .arg(tmp_src_dir.path())
        .assert()
        .code(2)
        .stderr("")
        .stdout(predicate::function(|stdout| {
            insta::assert_snapshot!(stdout);
            true
        }));
}

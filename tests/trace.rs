// Copyright 2023, 2024 Martin Pool

//! Tests for trace from the cargo-mutants CLI.

use predicates::prelude::*;

mod util;
use util::{has_testdata, run};

#[test]
fn env_var_controls_trace() {
    if !has_testdata() {
        return;
    }
    run()
        .env("CARGO_MUTANTS_TRACE_LEVEL", "trace")
        .args(["mutants", "--list"])
        .arg("-d")
        .arg("testdata/never_type")
        .assert()
        // This is a debug!() message; it should only be seen if the trace var
        // was wired correctly to stderr.
        .stderr(predicate::str::contains(
            "No mutants generated for this return type",
        ));
}

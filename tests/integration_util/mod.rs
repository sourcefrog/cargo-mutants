// Copyright 2024 Martin Pool

//! Utilities specific to integration tests that need access to the binary under test.

use std::env;

/// Create a Command to run cargo-mutants for integration tests.
pub fn run() -> assert_cmd::Command {
    let mut cmd = assert_cmd::Command::new(assert_cmd::cargo_bin!("cargo-mutants"));
    // Strip any options configured in the environment running these tests,
    // so that they don't cause unexpected behavior in the code under test.
    //
    // For example, without this,
    // `env CARGO_MUTANTS_JOBS=4 cargo mutants`
    //
    // would end up with tests running 4 jobs by default, which would cause
    // the tests to fail.
    //
    // Even more generally than that example, we want the tests to be as hermetic
    // as reasonably possible.
    //
    // Also strip GITHUB_ACTION to avoid automatically emitting github annotations,
    // so that tests are more hermetic and reproducible between local and CI.
    env::vars()
        .map(|(k, _v)| k)
        .filter(|k| {
            k.starts_with("CARGO_MUTANTS_")
                || k == "CLICOLOR_FORCE"
                || k == "NOCOLOR"
                || k == "CARGO_TERM_COLOR"
                || k == "GITHUB_ACTION"
        })
        .for_each(|k| {
            cmd.env_remove(k);
        });
    cmd
}

/// Returns the path to the cargo-mutants binary under test.
pub fn main_binary() -> &'static std::path::Path {
    assert_cmd::cargo_bin!("cargo-mutants")
}

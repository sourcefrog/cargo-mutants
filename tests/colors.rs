// Copyright 2024 Martin Pool

//! Tests for color output.

// Testing autodetection seems hard because we'd have to make a tty, so we'll rely on humans noticing
// for now.

use predicates::prelude::*;

mod util;
use util::{run, testdata_path};

fn has_color_listing() -> impl Predicate<str> {
    predicates::str::contains("with \x1b[33m0\x1b[0m")
}

fn has_ansi_escape() -> impl Predicate<str> {
    predicates::str::contains("\x1b[")
}

fn has_color_debug() -> impl Predicate<str> {
    predicates::str::contains("\x1b[34mDEBUG\x1b[0m")
}

/// The test fixtures force off colors, even if something else tries to turn it on.
#[test]
fn no_color_in_test_subprocesses_by_default() {
    let Some(path) = testdata_path("small_well_tested") else {
        return;
    };
    run()
        .args(["mutants", "--list"])
        .arg("-d")
        .arg(&path)
        .assert()
        .success()
        .stdout(has_ansi_escape().not())
        .stderr(has_ansi_escape().not());
}

/// Colors can be turned on with `--color` and they show up in the listing and
/// in trace output.
#[test]
fn colors_always_shows_in_stdout_and_trace() {
    let Some(dir) = testdata_path("small_well_tested") else {
        return;
    };
    run()
        .args(["mutants", "--list", "--colors=always", "-Ltrace"])
        .arg("-d")
        .arg(&dir)
        .assert()
        .success()
        .stdout(has_color_listing())
        .stderr(has_color_debug());
}

#[test]
fn cargo_term_color_env_shows_colors() {
    let Some(dir) = testdata_path("small_well_tested") else {
        return;
    };
    run()
        .env("CARGO_TERM_COLOR", "always")
        .args(["mutants", "--list", "-Ltrace", "-d"])
        .arg(dir)
        .assert()
        .success()
        .stdout(has_color_listing())
        .stderr(has_color_debug());
}

#[test]
fn invalid_cargo_term_color_rejected_with_message() {
    let Some(dir) = testdata_path("small_well_tested") else {
        return;
    };
    run()
        .env("CARGO_TERM_COLOR", "invalid")
        .args(["mutants", "--list", "-Ltrace", "-d"])
        .arg(dir)
        .assert()
        .stderr(predicate::str::contains(
            // The message does not currently name the variable due to <https://github.com/clap-rs/clap/issues/5202>.
            "invalid value 'invalid'",
        ))
        .code(1);
}

/// Colors can be turned on with `CLICOLOR_FORCE`.
#[test]
fn clicolor_force_shows_in_stdout_and_trace() {
    let Some(dir) = testdata_path("small_well_tested") else {
        return;
    };
    run()
        .env("CLICOLOR_FORCE", "1")
        .args(["mutants", "--list", "--colors=never", "-Ltrace", "-d"])
        .arg(dir)
        .assert()
        .success()
        .stdout(has_color_listing())
        .stderr(has_color_debug());
}

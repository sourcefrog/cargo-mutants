// Copyright 2024 Martin Pool

//! Tests for color output.

// Testing autodetection seems hard because we'd have to make a tty, so we'll rely on humans noticing
// for now.

use predicates::prelude::*;

use super::run;

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
    run()
        .args(["mutants", "-d", "testdata/small_well_tested", "--list"])
        .assert()
        .success()
        .stdout(has_ansi_escape().not())
        .stderr(has_ansi_escape().not());
}

/// Colors can be turned on with `--color` and they show up in the listing and
/// in trace output.
#[test]
fn colors_always_shows_in_stdout_and_trace() {
    run()
        .args([
            "mutants",
            "-d",
            "testdata/small_well_tested",
            "--list",
            "--colors=always",
            "-Ltrace",
        ])
        .assert()
        .success()
        .stdout(has_color_listing())
        .stderr(has_color_debug());
}

#[test]
fn cargo_term_color_env_shows_colors() {
    run()
        .env("CARGO_TERM_COLOR", "always")
        .args([
            "mutants",
            "-d",
            "testdata/small_well_tested",
            "--list",
            "-Ltrace",
        ])
        .assert()
        .success()
        .stdout(has_color_listing())
        .stderr(has_color_debug());
}

#[test]
fn invalid_cargo_term_color_rejected_with_message() {
    run()
        .env("CARGO_TERM_COLOR", "invalid")
        .args([
            "mutants",
            "-d",
            "testdata/small_well_tested",
            "--list",
            "-Ltrace",
        ])
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
    run()
        .env("CLICOLOR_FORCE", "1")
        .args([
            "mutants",
            "-d",
            "testdata/small_well_tested",
            "--list",
            "--colors=never",
            "-Ltrace",
        ])
        .assert()
        .success()
        .stdout(has_color_listing())
        .stderr(has_color_debug());
}

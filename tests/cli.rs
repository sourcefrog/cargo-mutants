// Copyright 2021 Martin Pool

//! Tests for CLI layer.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use itertools::Itertools;
// use assert_cmd::prelude::*;
// use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::{tempdir, TempDir};

use lazy_static::lazy_static;

use pretty_assertions::assert_eq;

lazy_static! {
    static ref MAIN_BINARY: PathBuf = assert_cmd::cargo::cargo_bin("cargo-mutants");
}

fn run_assert_cmd() -> assert_cmd::Command {
    assert_cmd::Command::new(MAIN_BINARY.as_os_str())
}

fn run() -> std::process::Command {
    Command::new(MAIN_BINARY.as_os_str())
}

trait CommandInstaExt {
    fn assert_insta(&mut self);
}

impl CommandInstaExt for std::process::Command {
    fn assert_insta(&mut self) {
        let output = self.output().expect("command completes");
        assert!(output.status.success());
        insta::assert_snapshot!(String::from_utf8_lossy(&output.stdout));
        assert_eq!(&String::from_utf8_lossy(&output.stderr), "");
    }
}

#[test]
fn detect_incorrect_cargo_subcommand() {
    // argv[1] "mutants" is missing here.
    run_assert_cmd().arg("wibble").assert().code(1);
}

#[test]
fn detect_missing_cargo_subcommand() {
    // argv[1] "mutants" is missing here.
    run_assert_cmd().assert().code(1);
}

#[test]
fn detect_option_in_place_of_cargo_subcommand() {
    // argv[1] "mutants" is missing here.
    run_assert_cmd().args(["--list"]).assert().code(1);
}

#[test]
fn uses_cargo_env_var_to_run_cargo_so_invalid_value_fails() {
    let bogus_cargo = "NOTHING_NONEXISTENT_VOID";
    run_assert_cmd()
        .env("CARGO", bogus_cargo)
        .args(["mutants", "-d", "testdata/tree/well_tested"])
        .assert()
        .stderr(
            (predicates::str::contains("No such file or directory").or(predicates::str::contains(
                "The system cannot find the file specified",
            )))
            .and(predicates::str::contains(bogus_cargo)),
        )
        .code(1);
    // TODO: Preferably there would be a more specific exit code for the
    // clean build failing.
}

#[test]
fn list_diff_json_not_yet_supported() {
    run_assert_cmd()
        .args(["mutants", "--list", "--json", "--diff"])
        .assert()
        .code(1)
        .stderr("--list --diff --json is not (yet) supported\n")
        .stdout("");
}

#[test]
fn list_mutants_in_factorial() {
    run()
        .arg("mutants")
        .arg("--list")
        .current_dir("testdata/tree/factorial")
        .assert_insta();
}

#[test]
fn list_mutants_in_factorial_json() {
    run()
        .arg("mutants")
        .arg("--list")
        .arg("--json")
        .current_dir("testdata/tree/factorial")
        .assert_insta();
}

#[test]
fn list_mutants_with_dir_option() {
    run()
        .arg("mutants")
        .arg("--list")
        .arg("--dir")
        .arg("testdata/tree/factorial")
        .assert_insta();
}

#[test]
fn list_mutants_with_diffs_in_factorial() {
    run()
        .arg("mutants")
        .arg("--list")
        .arg("--diff")
        .current_dir("testdata/tree/factorial")
        .assert_insta();
}

#[test]
fn list_mutants_well_tested() {
    run()
        .arg("mutants")
        .arg("--list")
        .current_dir("testdata/tree/well_tested")
        .assert_insta();
}

#[test]
fn list_mutants_json_well_tested() {
    run()
        .arg("mutants")
        .arg("--list")
        .arg("--json")
        .current_dir("testdata/tree/well_tested")
        .assert_insta();
}

// Copy the source because output is written into target/mutants.
fn copy_of_testdata(tree_name: &str) -> TempDir {
    let tmp_src_dir = tempdir().unwrap();
    cp_r::CopyOptions::new()
        .copy_tree(Path::new("testdata/tree").join(tree_name), &tmp_src_dir)
        .unwrap();
    tmp_src_dir
}

#[test]
fn well_tested_tree_finds_no_problems() {
    let tmp_src_dir = copy_of_testdata("well_tested");
    run_assert_cmd()
        .arg("mutants")
        .current_dir(&tmp_src_dir.path())
        .assert()
        .success();
    // TODO: Check some structured output or summary json?
}

#[test]
fn test_factorial() {
    let tmp_src_dir = copy_of_testdata("factorial");

    let output_re = r"copy source to scratch directory \.\.\. \d+ MB in \d\.\d\d\ds
baseline test with no mutations \.\.\. ok in \d+\.\d\d\ds
src/bin/main\.rs:1: replace main with \(\) \.\.\. NOT CAUGHT in \d+\.\d\d\ds
src/bin/main\.rs:7: replace factorial with Default::default\(\) \.\.\. caught in \d+\.\d\d\ds
";

    run_assert_cmd()
        .arg("mutants")
        .arg("-d")
        .arg(&tmp_src_dir.path())
        .assert()
        .code(2)
        .stderr("")
        .stdout(predicate::str::is_match(output_re).unwrap());

    // Some log files should have been created
    let log_dir = tmp_src_dir.path().join("mutants.out/log");
    assert!(log_dir.is_dir());

    let mut names = fs::read_dir(log_dir)
        .unwrap()
        .map(Result::unwrap)
        .map(|e| e.file_name().into_string().unwrap())
        .collect_vec();
    names.sort_unstable();

    insta::assert_debug_snapshot!("factorial__log_names", &names);
}

#[test]
fn factorial_mutants_with_all_logs() {
    // Skip the details of the cargo output, which is very unpredictable, but it should exist.
    let output_re = r"copy source to scratch directory \.\.\. \d MB in \d\.\d\d\ds
baseline test with no mutations \.\.\. ok in \d\.\d\d\ds
(?s).*
src/bin/main\.rs:1: replace main with \(\) \.\.\. NOT CAUGHT in \d\.\d\d\ds
.*
src/bin/main\.rs:7: replace factorial with Default::default\(\) \.\.\. caught in \d\.\d\d\ds
.*
";

    let tmp_src_dir = copy_of_testdata("factorial");
    run_assert_cmd()
        .arg("mutants")
        .arg("--all-logs")
        .arg("-d")
        .arg(&tmp_src_dir.path())
        .assert()
        .code(2)
        .stderr("")
        .stdout(predicate::str::is_match(output_re).unwrap());
}

#[test]
fn detect_already_failing_tests() {
    // The detailed text output contains some noisy parts
    let tmp_src_dir = copy_of_testdata("already_failing_tests");
    run_assert_cmd()
        .arg("mutants")
        .current_dir(&tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .assert()
        .code(4)
        .stdout(
            predicate::str::contains("running 1 test\ntest test_factorial ... FAILED").normalize(),
        )
        .stdout(
            predicate::str::contains(
                "thread 'test_factorial' panicked at 'assertion failed: `(left == right)`
  left: `720`,
 right: `72`'",
            )
            .normalize(),
        )
        .stdout(predicate::str::contains("lib.rs:11:5"))
        .stdout(predicate::str::contains(
            "tests failed in a clean copy of the tree, so no mutants were tested",
        ))
        .stdout(predicate::str::contains("test result: FAILED. 0 passed; 1 failed;").normalize());
}

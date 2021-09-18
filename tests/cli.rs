// Copyright 2021 Martin Pool

//! Tests for CLI layer.

use std::path::PathBuf;
use std::process::Command;

// use assert_cmd::prelude::*;
// use assert_cmd::Command;
use predicates::prelude::*;

use lazy_static::lazy_static;

#[allow(unused)]
use pretty_assertions::assert_eq;

lazy_static! {
    static ref MAIN_BINARY: PathBuf = assert_cmd::cargo::cargo_bin("enucleate");
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
fn list_files_in_factorial() {
    run()
        .arg("list-files")
        .arg("-d")
        .arg("testdata/tree/factorial")
        .assert_insta();
}


#[test]
fn list_files_in_cwd() {
    run()
        .arg("list-files")
        .current_dir("testdata/tree/factorial")
        .assert_insta();
}
#[test]
fn list_mutants_in_factorial() {
    run()
        .arg("list-mutants")
        .current_dir("testdata/tree/factorial")
        .assert_insta();
}

#[test]
fn list_mutants_with_dir_option() {
    run()
        .arg("list-mutants")
        .arg("--dir")
        .arg("testdata/tree/factorial")
        .assert_insta();
}

#[test]
fn list_mutants_with_diffs_in_factorial() {
    run()
        .arg("list-mutants")
        .arg("--diff")
        .current_dir("testdata/tree/factorial")
        .assert_insta();
}

#[test]
fn test_factorial() {
    run()
        .arg("test")
        .current_dir("testdata/tree/factorial")
        .assert_insta();
}

#[test]
fn detect_already_failing_tests() {
    // The detailed text output contains some noisy parts
    run_assert_cmd()
        .arg("test")
        .current_dir("testdata/tree/already_failing_tests")
        .env_remove("RUST_BACKTRACE")
        .assert()
        .failure()
        .stderr("Error: build in clean tree failed\n")
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
        .stdout(predicate::str::contains("test result: FAILED. 0 passed; 1 failed;").normalize());
}

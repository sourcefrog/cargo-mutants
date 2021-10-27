// Copyright 2021 Martin Pool

//! Tests for CLI layer.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use itertools::Itertools;
// use assert_cmd::prelude::*;
// use assert_cmd::Command;
use predicates::prelude::*;

use lazy_static::lazy_static;

#[allow(unused)]
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
fn list_mutants_in_factorial() {
    run()
        .arg("mutants")
        .arg("--list-mutants")
        .current_dir("testdata/tree/factorial")
        .assert_insta();
}

#[test]
fn list_mutants_with_dir_option() {
    run()
        .arg("mutants")
        .arg("--list-mutants")
        .arg("--dir")
        .arg("testdata/tree/factorial")
        .assert_insta();
}

#[test]
fn list_mutants_with_diffs_in_factorial() {
    run()
        .arg("mutants")
        .arg("--list-mutants")
        .arg("--diff")
        .current_dir("testdata/tree/factorial")
        .assert_insta();
}

#[test]
fn list_mutants_with_for_testdata_tree_sites() {
    run()
        .arg("mutants")
        .arg("--list-mutants")
        .current_dir("testdata/tree/sites")
        .assert_insta();
}

#[test]
fn test_factorial() {
    // TODO: This writes logs into the testdata directory, which is not ideal...
    let tree = Path::new("testdata/tree/factorial");
    let output = run().arg("mutants").current_dir(tree).output().unwrap();
    assert!(output.status.success());
    let remove_time = regex::Regex::new(r"in \d+\.\d{3}s").unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let cleaned_stdout = remove_time.replace_all(&stdout, "in x.xxxs");
    insta::assert_snapshot!(cleaned_stdout);
    assert_eq!(&String::from_utf8_lossy(&output.stderr), "");

    // Some log files should have been created
    let log_dir = tree.join("target/mutants/log");
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
fn detect_already_failing_tests() {
    // The detailed text output contains some noisy parts
    let tree = Path::new("testdata/tree/already_failing_tests");
    run_assert_cmd()
        .arg("mutants")
        .current_dir(tree)
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

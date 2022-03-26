// Copyright 2021, 2022 Martin Pool

//! Tests for cargo-mutants CLI layer.

use std::fs::{self, read_dir};
use std::path::{Path, PathBuf};
use std::process::Command;

use itertools::Itertools;
// use assert_cmd::prelude::*;
// use assert_cmd::Command;
use lazy_static::lazy_static;
use predicate::str::{contains, is_match};
use predicates::prelude::*;
use pretty_assertions::assert_eq;
use regex::Regex;
use tempfile::{tempdir, TempDir};

lazy_static! {
    static ref MAIN_BINARY: PathBuf = assert_cmd::cargo::cargo_bin("cargo-mutants");
    static ref DURATION_RE: Regex = Regex::new(r"\d+\.\d{1,3}s").unwrap();
    static ref SIZE_RE: Regex = Regex::new(r"\d+ MB").unwrap();
}

fn run_assert_cmd() -> assert_cmd::Command {
    assert_cmd::Command::new(MAIN_BINARY.as_os_str())
}

fn run() -> std::process::Command {
    Command::new(MAIN_BINARY.as_os_str())
}

trait CommandInstaExt {
    fn assert_insta(&mut self, snapshot_name: &str);
}

impl CommandInstaExt for std::process::Command {
    fn assert_insta(&mut self, snapshot_name: &str) {
        let output = self.output().expect("command completes");
        assert!(output.status.success());
        insta::assert_snapshot!(snapshot_name, String::from_utf8_lossy(&output.stdout));
        assert_eq!(&String::from_utf8_lossy(&output.stderr), "");
    }
}

// Copy the source because output is written into mutants.out.
fn copy_of_testdata(tree_name: &str) -> TempDir {
    let tmp_src_dir = tempdir().unwrap();
    cp_r::CopyOptions::new()
        .filter(|path, _stat| {
            Ok(["target", "mutants.out", "mutants.out.old"]
                .iter()
                .all(|p| !path.starts_with(p)))
        })
        .copy_tree(Path::new("testdata/tree").join(tree_name), &tmp_src_dir)
        .unwrap();
    tmp_src_dir
}

/// Remove anything that looks like a duration or tree size, since they'll be unpredictable.
fn redact_timestamps_sizes(s: &str) -> String {
    // TODO: Maybe match the number of digits?
    let s = DURATION_RE.replace_all(s, "x.xxxs");
    SIZE_RE.replace_all(&s, "xxx MB").to_string()
}

#[test]
fn incorrect_cargo_subcommand() {
    // argv[1] "mutants" is missing here.
    run_assert_cmd().arg("wibble").assert().code(1);
}

#[test]
fn missing_cargo_subcommand() {
    // argv[1] "mutants" is missing here.
    run_assert_cmd().assert().code(1);
}

#[test]
fn option_in_place_of_cargo_subcommand() {
    // argv[1] "mutants" is missing here.
    run_assert_cmd().args(["--list"]).assert().code(1);
}

#[test]
fn uses_cargo_env_var_to_run_cargo_so_invalid_value_fails() {
    let tmp_src_dir = copy_of_testdata("well_tested");
    let bogus_cargo = "NOTHING_NONEXISTENT_VOID";
    run_assert_cmd()
        .env("CARGO", bogus_cargo)
        .args(["mutants", "-d"])
        .arg(tmp_src_dir.path())
        .assert()
        .stderr(
            (predicates::str::contains("No such file or directory")
                .or(predicates::str::contains(
                    "The system cannot find the file specified",
                ))
                .or(
                    predicates::str::contains("program not found"), /* Windows */
                ))
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
        .assert_insta("list_mutants_in_factorial");
}

#[test]
fn list_mutants_in_factorial_json() {
    run()
        .arg("mutants")
        .arg("--list")
        .arg("--json")
        .current_dir("testdata/tree/factorial")
        .assert_insta("list_mutants_in_factorial_json");
}

#[test]
fn list_mutants_with_dir_option() {
    run()
        .arg("mutants")
        .arg("--list")
        .arg("--dir")
        .arg("testdata/tree/factorial")
        .assert_insta("list_mutants_with_dir_option");
}

#[test]
fn list_mutants_with_diffs_in_factorial() {
    run()
        .arg("mutants")
        .arg("--list")
        .arg("--diff")
        .current_dir("testdata/tree/factorial")
        .assert_insta("list_mutants_with_diffs_in_factorial");
}

#[test]
fn list_mutants_well_tested() {
    run()
        .arg("mutants")
        .arg("--list")
        .current_dir("testdata/tree/well_tested")
        .assert_insta("list_mutants_well_tested");
}

#[test]
fn list_mutants_json_well_tested() {
    run()
        .arg("mutants")
        .arg("--list")
        .arg("--json")
        .current_dir("testdata/tree/well_tested")
        .assert_insta("list_mutants_json_well_tested");
}

#[test]
fn copy_testdata_doesnt_include_build_artifacts() {
    // If there is a target or mutants.out in the source directory, we don't want it in the copy,
    // so that the tests are (more) hermetic.
    let tmp_src_dir = copy_of_testdata("factorial");
    assert!(!tmp_src_dir.path().join("mutants.out").exists());
    assert!(!tmp_src_dir.path().join("target").exists());
    assert!(!tmp_src_dir.path().join("mutants.out.old").exists());
    assert!(tmp_src_dir.path().join("Cargo.toml").exists());
}

#[test]
fn well_tested_tree_quiet() {
    let tmp_src_dir = copy_of_testdata("well_tested");
    run_assert_cmd()
        .arg("mutants")
        .arg("--no-times")
        .arg("--no-shuffle")
        .current_dir(&tmp_src_dir.path())
        .assert()
        .success()
        .stdout(predicate::function(|stdout| {
            insta::assert_snapshot!(stdout);
            true
        }));
    // The format of outcomes.json is not pinned down yet, but it should exist.
    assert!(tmp_src_dir
        .path()
        .join("mutants.out/outcomes.json")
        .exists());
}

#[test]
fn well_tested_tree_finds_no_problems() {
    let tmp_src_dir = copy_of_testdata("well_tested");
    run_assert_cmd()
        .arg("mutants")
        .arg("--no-times")
        .arg("--caught")
        .arg("--no-shuffle")
        .current_dir(&tmp_src_dir.path())
        .assert()
        .success()
        .stdout(predicate::function(|stdout| {
            insta::assert_snapshot!(stdout);
            true
        }));
    // The format of outcomes.json is not pinned down yet, but it should exist.
    assert!(tmp_src_dir
        .path()
        .join("mutants.out/outcomes.json")
        .exists());
}

#[test]
fn well_tested_tree_check_only() {
    let tmp_src_dir = copy_of_testdata("well_tested");
    run_assert_cmd()
        .args(["mutants", "--check", "--no-shuffle", "--no-times"])
        .current_dir(&tmp_src_dir.path())
        .assert()
        .success()
        .stdout(predicate::function(|stdout| {
            insta::assert_snapshot!(stdout);
            true
        }));
}

#[test]
fn well_tested_tree_check_only_shuffled() {
    let tmp_src_dir = copy_of_testdata("well_tested");
    run_assert_cmd()
        .args(["mutants", "--check", "--no-times", "--shuffle"])
        .current_dir(&tmp_src_dir.path())
        .assert()
        .success();
    // Caution: No assertions about output here, we just check that it runs.
}

#[test]
fn uncaught_mutant_in_factorial() {
    let tmp_src_dir = copy_of_testdata("factorial");

    run_assert_cmd()
        .arg("mutants")
        .arg("--no-shuffle")
        .arg("-d")
        .arg(&tmp_src_dir.path())
        .assert()
        .code(2)
        .stderr("")
        .stdout(predicate::function(|stdout| {
            insta::assert_snapshot!(redact_timestamps_sizes(stdout));
            true
        }));

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

    // A mutants.json is in the mutants.out directory.
    let mutants_json =
        fs::read_to_string(tmp_src_dir.path().join("mutants.out/mutants.json")).unwrap();
    insta::assert_snapshot!("mutants.json", mutants_json);
}

#[test]
fn factorial_mutants_with_all_logs() {
    // The log contains a lot of build output, which is hard to deal with, but let's check that
    // some key lines are there.
    let tmp_src_dir = copy_of_testdata("factorial");
    run_assert_cmd()
        .arg("mutants")
        .arg("--all-logs")
        .arg("-v")
        .arg("-V")
        .arg("-d")
        .arg(&tmp_src_dir.path())
        .assert()
        .code(2)
        .stderr("")
        .stdout(is_match(r"source tree \.\.\. ok in \d+\.\d\d\ds").unwrap())
        .stdout(is_match(
r"Copy source and build products to scratch directory \.\.\. \d+ MB in \d+\.\d\d\ds"
        ).unwrap())
        .stdout(is_match(
r"Unmutated baseline \.\.\. ok in \d+\.\d\d\ds"
        ).unwrap())
        .stdout(is_match(
r"src/bin/main\.rs:1: replace main with \(\) \.\.\. NOT CAUGHT in \d+\.\d\d\ds"
        ).unwrap())
        .stdout(is_match(
r"src/bin/main\.rs:7: replace factorial -> u32 with Default::default\(\) \.\.\. caught in \d+\.\d\d\ds"
        ).unwrap());
}

#[test]
fn factorial_mutants_with_all_logs_and_nocapture() {
    let tmp_src_dir = copy_of_testdata("factorial");
    run_assert_cmd()
        .arg("mutants")
        .args(["--all-logs", "-v", "-V"])
        .arg("-d")
        .arg(&tmp_src_dir.path())
        .args(["--", "--", "--nocapture"])
        .assert()
        .code(2)
        .stderr("")
        .stdout(contains("factorial(6) = 720")) // println from the test
        .stdout(contains("factorial(6) = 0")) // The mutated result
        ;
}

#[test]
fn factorial_mutants_no_copy_target() {
    let tmp_src_dir = copy_of_testdata("factorial");
    run_assert_cmd()
        .arg("mutants")
        .args(["--no-copy-target", "--no-times"])
        .arg("-d")
        .arg(&tmp_src_dir.path())
        .assert()
        .code(2)
        .stderr("")
        .stdout(predicate::function(|stdout| {
            insta::assert_snapshot!(stdout);
            true
        }));
}

#[test]
fn check_succeeds_in_tree_that_builds_but_fails_tests() {
    // --check doesn't actually run the tests so won't discover that they fail.
    let tmp_src_dir = copy_of_testdata("already_failing_tests");
    run_assert_cmd()
        .arg("mutants")
        .arg("--check")
        .arg("--no-times")
        .current_dir(&tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .assert()
        .success()
        .stdout(predicate::function(|stdout| {
            insta::assert_snapshot!(stdout);
            true
        }));
}

#[test]
fn check_tree_with_mutants_skip() {
    let tmp_src_dir = copy_of_testdata("hang_avoided_by_attr");
    run_assert_cmd()
        .arg("mutants")
        .arg("--check")
        .arg("--no-times")
        .current_dir(&tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .assert()
        .success()
        .stdout(predicate::function(|stdout| {
            insta::assert_snapshot!(stdout);
            true
        }));
}

#[test]
fn already_failing_tests_are_detected_before_running_mutants() {
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
            "cargo test failed in an unmutated tree, so no mutants were tested",
        ))
        .stdout(predicate::str::contains("test result: FAILED. 0 passed; 1 failed;").normalize());
}

#[test]
fn already_failing_doctests_are_detected() {
    let tmp_src_dir = copy_of_testdata("already_failing_doctests");
    run_assert_cmd()
        .arg("mutants")
        .current_dir(&tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .assert()
        .code(4) // CLEAN_TESTS_FAILED
        .stdout(contains("lib.rs - takes_one_arg (line 5) ... FAILED"))
        .stdout(contains(
            "this function takes 1 argument but 3 arguments were supplied",
        ))
        .stdout(predicate::str::contains(
            "cargo test failed in an unmutated tree, so no mutants were tested",
        ));
}

#[test]
fn already_failing_doctests_can_be_skipped_with_cargo_arg() {
    let tmp_src_dir = copy_of_testdata("already_failing_doctests");
    run_assert_cmd()
        .arg("mutants")
        .args(["--", "--all-targets"])
        .current_dir(&tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .assert()
        .code(0)
        .stdout(contains("Found 1 mutation to test"));
}

#[test]
fn source_tree_build_fails() {
    let tmp_src_dir = copy_of_testdata("build_fails");
    run_assert_cmd()
        .arg("mutants")
        .current_dir(&tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .assert()
        .failure() // TODO: This should be a distinct error code
        .stdout(is_match(r"source tree \.\.\. FAILED in \d+\.\d{3}s").unwrap())
        .stdout(contains(r"This isn't Rust").name("The problem source line"))
        .stdout(contains("*** source tree"))
        .stdout(contains("check --tests")) // Caught at the check phase
        .stdout(contains("lib.rs:1:6"))
        .stdout(contains("*** cargo result: "))
        .stdout(contains("check failed in source tree, not continuing"));
}

#[test]
fn timeout_when_unmutated_tree_test_hangs() {
    let tmp_src_dir = copy_of_testdata("already_hangs");
    run_assert_cmd()
        .arg("mutants")
        .args(["--timeout", "2.9"])
        .current_dir(&tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .assert()
        .code(4) // exit_code::CLEAN_TESTS_FAILED
        .stdout(is_match(r"Unmutated baseline \.\.\. TIMEOUT in \d+\.\d{3}s").unwrap())
        .stdout(contains("timeout"))
        .stdout(contains(
            "cargo test failed in an unmutated tree, so no mutants were tested",
        ));
}

#[test]
fn hang_when_mutated() {
    let tmp_src_dir = copy_of_testdata("hang_when_mutated");
    run_assert_cmd()
        .arg("mutants")
        .args(["-t", "1.9"])
        .current_dir(&tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .assert()
        .code(3) // exit_code::TIMEOUT
        .stdout(contains(
            "replace should_stop -> bool with false ... TIMEOUT",
        ))
        .stdout(contains(
            "replace should_stop -> bool with true ... NOT CAUGHT",
        ));
}

#[test]
fn log_file_names_are_short_and_dont_collide() {
    // The "well-tested" tree can generate multiple mutants from single lines. They get distinct file names.
    let tmp_src_dir = copy_of_testdata("well_tested");
    let cmd_assert = run_assert_cmd()
        .arg("mutants")
        .args(["--check", "-v", "-V"])
        .current_dir(&tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .assert()
        .success();
    println!(
        "{}",
        String::from_utf8_lossy(&cmd_assert.get_output().stdout)
    );
    let log_dir = tmp_src_dir.path().join("mutants.out").join("log");
    let all_log_names = read_dir(&log_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_str().unwrap().to_string())
        .inspect(|filename| println!("{}", filename))
        .collect::<Vec<_>>();
    assert!(all_log_names.len() > 10);
    assert!(
        all_log_names.iter().all(|filename| filename.len() < 80),
        "log file names are too long"
    );
    assert!(
        all_log_names
            .iter()
            .any(|filename| filename.ends_with("_001.log")),
        "log file names are not disambiguated"
    );
}

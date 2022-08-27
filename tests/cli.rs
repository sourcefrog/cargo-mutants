// Copyright 2021, 2022 Martin Pool

//! Tests for cargo-mutants CLI layer.

use std::fmt::Write;
use std::fs::{self, read_dir};
use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::prelude::OutputAssertExt;
use itertools::Itertools;
// use assert_cmd::prelude::*;
// use assert_cmd::Command;
use lazy_static::lazy_static;
use path_slash::PathBufExt;
use predicate::str::{contains, is_match};
use predicates::prelude::*;
use pretty_assertions::assert_eq;
use regex::Regex;
use tempfile::{tempdir, TempDir};

lazy_static! {
    static ref MAIN_BINARY: PathBuf = assert_cmd::cargo::cargo_bin("cargo-mutants");
    static ref DURATION_RE: Regex = Regex::new(r"(\d+\.\d{1,3}s|\d+:\d{2})").unwrap();
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
fn show_version() {
    run_assert_cmd()
        .args(["mutants", "--version"])
        .assert()
        .success()
        .stdout(predicates::str::is_match(r"^cargo-mutants \d+\.\d+\.\d+(-.*)?\n$").unwrap());
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
            predicates::str::contains("No such file or directory")
                .or(predicates::str::contains(
                    "The system cannot find the file specified",
                ))
                .or(
                    predicates::str::contains("program not found"), /* Windows */
                ),
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

/// Return paths to all testdata trees, in order, excluding leftover git
/// detritus with no Cargo.toml.
fn all_testdata_tree_paths() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = fs::read_dir("testdata/tree")
        .unwrap()
        .map(|r| r.unwrap())
        .filter(|dir_entry| dir_entry.file_type().unwrap().is_dir())
        .filter(|dir_entry| dir_entry.file_name() != "parse_fails")
        .map(|dir_entry| dir_entry.path())
        .filter(|dir_path| dir_path.join("Cargo.toml").exists())
        .collect();
    paths.sort();
    paths
}

#[test]
fn list_mutants_in_all_trees_as_json() {
    // The snapshot accumulated here is actually a big text file
    // containing JSON fragments. This might seem a bit weird for easier
    // review I want just a single snapshot, and json-inside-json has quoting
    // that makes it harder to review.
    let mut buf = String::new();
    for dir_path in all_testdata_tree_paths() {
        writeln!(buf, "## {}\n", dir_path.to_slash_lossy()).unwrap();
        let cmd_assert = run()
            .arg("mutants")
            .arg("--list")
            .arg("--json")
            .current_dir(&dir_path)
            .assert()
            .success();
        let json_str = String::from_utf8_lossy(&cmd_assert.get_output().stdout);
        writeln!(buf, "```json\n{}\n```\n", json_str).unwrap();
    }
    insta::assert_snapshot!(buf);
}

#[test]
fn list_mutants_in_all_trees_as_text() {
    let mut buf = String::new();
    for dir_path in all_testdata_tree_paths() {
        writeln!(buf, "## {}\n\n```", dir_path.to_slash_lossy()).unwrap();
        let stdout = run()
            .arg("mutants")
            .arg("--list")
            .current_dir(&dir_path)
            .output()
            .unwrap()
            .stdout;
        buf.push_str(&String::from_utf8_lossy(&stdout));
        buf.push_str("```\n\n");
    }
    insta::assert_snapshot!(buf);
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
fn list_mutants_in_cfg_attr_mutants_skip() {
    let tmp_src_dir = copy_of_testdata("cfg_attr_mutants_skip");
    run()
        .arg("mutants")
        .arg("--list")
        .current_dir(tmp_src_dir.path())
        .assert_insta("list_mutants_in_cfg_attr_mutants_skip");
}

#[test]
fn list_mutants_in_cfg_attr_mutants_skip_json() {
    let tmp_src_dir = copy_of_testdata("cfg_attr_mutants_skip");
    run()
        .arg("mutants")
        .arg("--list")
        .arg("--json")
        .current_dir(tmp_src_dir.path())
        .assert_insta("list_mutants_in_cfg_attr_mutants_skip_json");
}

#[test]
fn list_mutants_in_cfg_attr_test_skip() {
    let tmp_src_dir = copy_of_testdata("cfg_attr_test_skip");
    run()
        .arg("mutants")
        .arg("--list")
        .current_dir(tmp_src_dir.path())
        .assert_insta("list_mutants_in_cfg_attr_test_skip");
}

#[test]
fn list_mutants_in_cfg_attr_test_skip_json() {
    let tmp_src_dir = copy_of_testdata("cfg_attr_test_skip");
    run()
        .arg("mutants")
        .arg("--list")
        .arg("--json")
        .current_dir(tmp_src_dir.path())
        .assert_insta("list_mutants_in_cfg_attr_test_skip_json");
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
fn list_mutants_well_tested_examine_name_filter() {
    run()
        .arg("mutants")
        .args(["--list", "--file", "nested_function.rs"])
        .current_dir("testdata/tree/well_tested")
        .assert_insta("list_mutants_well_tested_examine_name_filter");
}

#[test]
fn list_mutants_well_tested_exclude_name_filter() {
    run()
        .arg("mutants")
        .args(["--list", "--exclude", "simple_fns.rs"])
        .current_dir("testdata/tree/well_tested")
        .assert_insta("list_mutants_well_tested_exclude_name_filter");
}

#[test]
fn list_mutants_well_tested_exclude_folder_filter() {
    run()
        .arg("mutants")
        .args(["--list", "--exclude", "*/module/*"])
        .current_dir("testdata/tree/with_child_directories")
        .assert_insta("list_mutants_well_tested_exclude_folder_filter");
}

#[test]
#[cfg(target_os = "windows")]
fn list_mutants_well_tested_exclude_folder_containing_backslash_on_windows() {
    run()
        .arg("mutants")
        .args(["--list", "--exclude", "*\\module\\*"])
        .current_dir("testdata/tree/with_child_directories")
        .assert_insta("list_mutants_well_tested_exclude_folder_filter");
}

#[test]
fn list_mutants_well_tested_examine_and_exclude_name_filter_combined() {
    run()
        .arg("mutants")
        .args([
            "--list",
            "--file",
            "src/module/utils/*.rs",
            "--exclude",
            "nested_function.rs",
        ])
        .current_dir("testdata/tree/with_child_directories")
        .assert_insta("list_mutants_well_tested_examine_and_exclude_name_filter_combined");
}

#[test]
fn tree_with_child_directories_is_well_tested() {
    let tmp_src_dir = copy_of_testdata("with_child_directories");
    run()
        .arg("mutants")
        .arg("-d")
        .arg(tmp_src_dir.path())
        .assert()
        .success();
}

#[test]
fn list_mutants_well_tested_multiple_examine_and_exclude_name_filter_with_files_and_folders() {
    run()
        .arg("mutants")
        .args(["--list", "--file", "module_methods.rs", "--file", "*/utils/*", "--exclude", "*/sub_utils/*", "--exclude", "nested_function.rs"])
        .current_dir("testdata/tree/with_child_directories")
        .assert_insta("list_mutants_well_tested_multiple_examine_and_exclude_name_filter_with_files_and_folders");
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
fn list_files_text_well_tested() {
    run()
        .arg("mutants")
        .arg("--list-files")
        .current_dir("testdata/tree/well_tested")
        .assert_insta("list_files_text_well_tested");
}

#[test]
fn list_files_json_well_tested() {
    run()
        .arg("mutants")
        .arg("--list-files")
        .arg("--json")
        .current_dir("testdata/tree/well_tested")
        .assert_insta("list_files_json_well_tested");
}

#[test]
fn list_files_json_workspace() {
    // Demonstrates that we get package names in the json listing.
    run()
        .args(["mutants", "--list-files", "--json"])
        .current_dir("testdata/tree/workspace")
        .assert_insta("list_files_json_workspace");
}

#[test]
fn workspace_tree_is_well_tested() {
    let tmp_src_dir = copy_of_testdata("workspace");
    run()
        .args(["mutants", "-d"])
        .arg(tmp_src_dir.path())
        .assert()
        .success();
    // The outcomes.json has some summary data
    let json_str =
        fs::read_to_string(tmp_src_dir.path().join("mutants.out/outcomes.json")).unwrap();
    println!("outcomes.json:\n{}", json_str);
    let json: serde_json::Value = json_str.parse().unwrap();
    assert_eq!(json["total_mutants"].as_u64().unwrap(), 3);
    assert_eq!(json["caught"].as_u64().unwrap(), 3);
    assert_eq!(json["missed"].as_u64().unwrap(), 0);
    assert_eq!(json["timeout"].as_u64().unwrap(), 0);
    let outcomes = json["outcomes"].as_array().unwrap();

    {
        let sourcetree_json = outcomes[0].as_object().expect("outcomes[0] is an object");
        assert_eq!(sourcetree_json["scenario"].as_str().unwrap(), "SourceTree");
        assert_eq!(sourcetree_json["summary"], "Success");
        let sourcetree_phases = sourcetree_json["phase_results"].as_array().unwrap();
        assert_eq!(sourcetree_phases.len(), 1);
        let sourcetree_command = sourcetree_phases[0]["command"].as_array().unwrap();
        assert_eq!(sourcetree_command[1..], ["build", "--tests", "--workspace"]);
    }

    {
        let baseline = outcomes[1].as_object().unwrap();
        assert_eq!(baseline["scenario"].as_str().unwrap(), "Baseline");
        assert_eq!(baseline["summary"], "Success");
        let baseline_phases = baseline["phase_results"].as_array().unwrap();
        assert_eq!(baseline_phases.len(), 2);
        assert_eq!(baseline_phases[0]["cargo_result"], "Success");
        assert_eq!(
            baseline_phases[0]["command"].as_array().unwrap()[1..],
            ["build", "--tests", "--workspace"]
        );
        assert_eq!(baseline_phases[1]["cargo_result"], "Success");
        assert_eq!(
            baseline_phases[1]["command"].as_array().unwrap()[1..],
            ["test", "--workspace"]
        );
    }

    assert_eq!(outcomes.len(), 5);
    for outcome in &outcomes[2..] {
        let mutant = &outcome["scenario"]["Mutant"];
        let package_name = mutant["package"].as_str().unwrap();
        assert!(!package_name.is_empty());
        assert_eq!(outcome["summary"], "CaughtMutant");
        let mutant_phases = outcome["phase_results"].as_array().unwrap();
        assert_eq!(mutant_phases.len(), 2);
        assert_eq!(mutant_phases[0]["cargo_result"], "Success");
        assert_eq!(
            mutant_phases[0]["command"].as_array().unwrap()[1..],
            ["build", "--tests", "--package", package_name]
        );
        assert_eq!(mutant_phases[1]["cargo_result"], "Failure");
        assert_eq!(
            mutant_phases[1]["command"].as_array().unwrap()[1..],
            ["test", "--package", package_name],
        );
    }
    {
        let baseline = json["outcomes"][1].as_object().unwrap();
        assert_eq!(baseline["scenario"].as_str().unwrap(), "Baseline");
        assert_eq!(baseline["summary"], "Success");
        let baseline_phases = baseline["phase_results"].as_array().unwrap();
        assert_eq!(baseline_phases.len(), 2);
        assert_eq!(baseline_phases[0]["cargo_result"], "Success");
        assert_eq!(
            baseline_phases[0]["command"].as_array().unwrap()[1..],
            ["build", "--tests", "--workspace"]
        );
        assert_eq!(baseline_phases[1]["cargo_result"], "Success");
        assert_eq!(
            baseline_phases[1]["command"].as_array().unwrap()[1..],
            ["test", "--workspace"]
        );
    }
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
fn small_well_tested_tree_is_clean() {
    let tmp_src_dir = copy_of_testdata("small_well_tested");
    run_assert_cmd()
        .arg("mutants")
        .args(["--no-times", "--no-shuffle", "-v", "-V"])
        .current_dir(&tmp_src_dir.path())
        .assert()
        .success()
        .stdout(predicate::function(|stdout| {
            insta::assert_snapshot!(stdout);
            true
        }));
    // The log file should exist and include something that looks like a diff.
    let log_content = fs::read_to_string(
        tmp_src_dir
            .path()
            .join("mutants.out/log/src__lib.rs_line_4.log"),
    )
    .unwrap()
    .replace('\r', "");
    println!("log content:\n{}", log_content);
    assert!(log_content.contains("*** mutation diff"));
    assert!(log_content.contains(
        "\
*** mutation diff:
--- src/lib.rs
+++ replace factorial with Default::default()
@@ -1,17 +1,13 @@"
    ));
    assert!(log_content.contains(
        "\
 pub fn factorial(n: u32) -> u32 {
-    let mut a = 1;
-    for i in 2..=n {
-        a *= i;
-    }
-    a
+Default::default() /* ~ changed by cargo-mutants ~ */
 }
"
    ));
    // Also, it should contain output from the failed tests with mutations applied.
    assert!(log_content.contains("test test::test_factorial ... FAILED"));

    assert!(log_content.contains("---- test::test_factorial stdout ----"));
    assert!(log_content.contains("factorial(6) = 0"));
}

#[test]
fn cdylib_tree_is_well_tested() {
    let tmp_src_dir = copy_of_testdata("cdylib");
    run_assert_cmd()
        .arg("mutants")
        .args(["--no-times", "--no-shuffle", "-v", "-V"])
        .current_dir(&tmp_src_dir.path())
        .assert()
        .success()
        .stdout(predicate::function(|stdout| {
            insta::assert_snapshot!(stdout);
            true
        }));
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
    let outcomes_json =
        fs::read_to_string(tmp_src_dir.path().join("mutants.out/outcomes.json")).unwrap();
    println!("outcomes.json:\n{}", outcomes_json);
    let outcomes: serde_json::Value = outcomes_json.parse().unwrap();
    assert_eq!(outcomes["total_mutants"], 15);
    assert_eq!(outcomes["caught"], 15);
    assert_eq!(outcomes["unviable"], 0);
    assert_eq!(outcomes["missed"], 0);
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
fn error_when_no_mutants_found() {
    let tmp_src_dir = copy_of_testdata("everything_skipped");
    run_assert_cmd()
        .args(["mutants", "--check", "--no-times", "--no-shuffle"])
        .current_dir(&tmp_src_dir.path())
        .assert()
        .stderr(predicate::str::contains("Error: No mutants found"))
        .stdout(predicate::str::contains("Found 0 mutants to test"))
        .failure();
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
        .stdout(is_match(r"source tree \.\.\. ok in \d+\.\ds").unwrap())
        .stdout(is_match(
r"Copy source and build products to scratch directory \.\.\. \d+ MB in \d+\.\ds"
        ).unwrap())
        .stdout(is_match(
r"Unmutated baseline \.\.\. ok in \d+\.\ds"
        ).unwrap())
        .stdout(is_match(
r"src/bin/factorial\.rs:1: replace main with \(\) \.\.\. NOT CAUGHT in \d+\.\ds"
        ).unwrap())
        .stdout(is_match(
r"src/bin/factorial\.rs:7: replace factorial -> u32 with Default::default\(\) \.\.\. caught in \d+\.\ds"
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
fn small_well_tested_mutants_with_cargo_arg_release() {
    let tmp_src_dir = copy_of_testdata("small_well_tested");
    run_assert_cmd()
        .arg("mutants")
        .args(["--no-times", "--cargo-arg", "--release"])
        .arg("-d")
        .arg(&tmp_src_dir.path())
        .assert()
        .success()
        .stderr("")
        .stdout(predicate::function(|stdout| {
            insta::assert_snapshot!(stdout);
            true
        }));
    // Check that it was actually passed.
    let baseline_log_path = &tmp_src_dir.path().join("mutants.out/log/baseline.log");
    println!("{}", baseline_log_path.display());
    let log_content = fs::read_to_string(&baseline_log_path).unwrap();
    println!("{}", log_content);
    regex::Regex::new(r"cargo.* build --tests --workspace --release")
        .unwrap()
        .captures(&log_content)
        .unwrap();
    regex::Regex::new(r"cargo.* test --workspace --release")
        .unwrap()
        .captures(&log_content)
        .unwrap();
}

#[test]
fn output_option() {
    let tmp_src_dir = copy_of_testdata("factorial");
    let output_tmpdir = TempDir::new().unwrap();
    assert!(
        !tmp_src_dir.path().join("mutants.out").exists(),
        "mutants.out should not be in a clean copy of the test data"
    );
    run_assert_cmd()
        .arg("mutants")
        .arg("--output")
        .arg(&output_tmpdir.path())
        .args(["--check", "--no-times"])
        .arg("-d")
        .arg(&tmp_src_dir.path())
        .assert()
        .success();
    assert!(
        !tmp_src_dir.path().join("mutants.out").exists(),
        "mutants.out should not be in the source directory after --output was given"
    );
    assert!(
        output_tmpdir.path().join("mutants.out").exists(),
        "mutants.out is in --output directory"
    );
    assert!(
        output_tmpdir
            .path()
            .join("mutants.out/mutants.json")
            .is_file(),
        "mutants.out/mutants.json is in --output directory"
    );
    assert!(
        output_tmpdir
            .path()
            .join("mutants.out/outcomes.json")
            .is_file(),
        "mutants.out/outcomes.json is in --output directory"
    );
    assert!(
        output_tmpdir.path().join("mutants.out/debug.log").is_file(),
        "mutants.out/debug.log is in --output directory"
    );
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
        .stdout(contains("Found 1 mutant to test"));
}

#[test]
fn source_tree_parse_fails() {
    let tmp_src_dir = copy_of_testdata("parse_fails");
    run_assert_cmd()
        .arg("mutants")
        .current_dir(&tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .assert()
        .failure() // TODO: This should be a distinct error code
        .stdout(is_match(r"source tree \.\.\. FAILED in \d+\.\ds").unwrap())
        .stdout(contains(r#"This isn't Rust..."#).name("The problem source line"))
        .stdout(contains("*** source tree"))
        .stdout(contains("build --tests")) // Caught at the check phase
        .stdout(contains("lib.rs:3"))
        .stdout(contains("*** cargo result: "))
        .stdout(contains("build failed in source tree, not continuing"));
}

#[test]
fn source_tree_typecheck_fails() {
    let tmp_src_dir = copy_of_testdata("typecheck_fails");
    run_assert_cmd()
        .arg("mutants")
        .current_dir(&tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .assert()
        .failure() // TODO: This should be a distinct error code
        .stdout(is_match(r"source tree \.\.\. FAILED in \d+\.\ds").unwrap())
        .stdout(
            contains(r#""1" + 2 // Doesn't work in Rust: just as well!"#)
                .name("The problem source line"),
        )
        .stdout(contains("*** source tree"))
        .stdout(contains("build --tests")) // Caught at the check phase
        .stdout(contains("lib.rs:6"))
        .stdout(contains("*** cargo result: "))
        .stdout(contains("build failed in source tree, not continuing"));
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
        .stdout(is_match(r"Unmutated baseline \.\.\. TIMEOUT in \d+\.\ds").unwrap())
        .stdout(contains("timeout"))
        .stdout(contains(
            "cargo test failed in an unmutated tree, so no mutants were tested",
        ));
}

/// A tree that hangs when some functions are mutated does not hang cargo-mutants
/// overall, because we impose a timeout. The timeout can be specified on the
/// command line, with decimal seconds.
///
/// This test is a bit at risk of being flaky, because it depends on the progress
/// of real time and tests can be unexpectedly slow on CI.
///
/// The `hang_when_mutated` tree generates three mutants:
///
/// * `controlled_loop` could be replaced to return 0 and this will be
///   detected, because it should normally return at least one.
///
/// * `should_stop` could change to always return `true`, in which case
///   the test will fail and the mutant will be caught because the loop
///   does only one pass.
///
/// * `should_stop` could change to always return `false`, in which case
///   the loop will never stop, but the test should eventually be killed
///   by a timeout.
#[test]
fn hang_when_mutated_tree_does_not_hang_cargo_mutants_with_explicit_short_timeout() {
    let tmp_src_dir = copy_of_testdata("hang_when_mutated");
    // Also test that it accepts decimal seconds
    run_assert_cmd()
        .arg("mutants")
        .args(["-t", "4.1", "-v", "--", "--", "--nocapture"])
        .current_dir(&tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .assert()
        .code(3) // exit_code::TIMEOUT
        .stdout(contains(
            "replace should_stop -> bool with false ... TIMEOUT",
        ))
        .stdout(contains("replace should_stop -> bool with true ... caught"))
        .stdout(contains(
            "replace controlled_loop -> usize with Default::default() ... caught",
        ));
    // TODO: Inspect outcomes.json.
}

#[test]
fn log_file_names_are_short_and_dont_collide() {
    // The "well_tested" tree can generate multiple mutants from single lines. They get distinct file names.
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

#[test]
fn cargo_mutants_in_override_dependency_tree_passes() {
    // Run against the testdata directory directly, without copying it, so that the
    // relative dependency `../dependency` is still used. We also pass `--no-copy-target`
    // so that we don't do a freshening build first, so nothing should write into the
    // source directory.
    run_assert_cmd()
        .arg("mutants")
        .arg("--no-times")
        .arg("--no-copy-target")
        .arg("--no-shuffle")
        .arg("-d")
        .arg("testdata/tree/override_dependency")
        .assert()
        .success()
        .stdout(predicate::function(|stdout| {
            insta::assert_snapshot!(stdout);
            true
        }));
}

#[test]
fn cargo_mutants_in_relative_dependency_tree_passes() {
    // Run against the testdata directory directly, without copying it, so that the
    // relative dependency `../dependency` is still used. We also pass `--no-copy-target`
    // so that we don't do a freshening build first, so nothing should write into the
    // source directory.
    run_assert_cmd()
        .arg("mutants")
        .arg("--no-times")
        .arg("--no-copy-target")
        .arg("--no-shuffle")
        .arg("-d")
        .arg("testdata/tree/relative_dependency")
        .assert()
        .success()
        .stdout(predicate::function(|stdout| {
            insta::assert_snapshot!(stdout);
            true
        }));
}

#[test]
fn cargo_mutants_in_replace_dependency_tree_passes() {
    // Run against the testdata directory directly, without copying it, so that the
    // relative dependency `../dependency` is still used. We also pass `--no-copy-target`
    // so that we don't do a freshening build first, so nothing should write into the
    // source directory.
    run_assert_cmd()
        .arg("mutants")
        .arg("--no-times")
        .arg("--no-copy-target")
        .arg("--no-shuffle")
        .arg("-d")
        .arg("testdata/tree/replace_dependency")
        .assert()
        .success()
        .stdout(predicate::function(|stdout| {
            insta::assert_snapshot!(stdout);
            true
        }));
}

#[test]
fn cargo_mutants_in_patch_dependency_tree_passes() {
    // Run against the testdata directory directly, without copying it, so that the
    // relative dependency `../dependency` is still used. We also pass `--no-copy-target`
    // so that we don't do a freshening build first, so nothing should write into the
    // source directory.
    run_assert_cmd()
        .arg("mutants")
        .arg("--no-times")
        .arg("--no-copy-target")
        .arg("--no-shuffle")
        .arg("-d")
        .arg("testdata/tree/patch_dependency")
        .assert()
        .success()
        .stdout(predicate::function(|stdout| {
            insta::assert_snapshot!(stdout);
            true
        }));
}

#[test]
fn strict_warnings_about_unused_variables_are_disabled_so_mutants_compile() {
    let tmp_src_dir = copy_of_testdata("strict_warnings");
    run_assert_cmd()
        .arg("mutants")
        .arg("--check")
        .arg("--no-times")
        .current_dir(&tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .assert()
        .success()
        .stdout(contains("1 mutant tested: 1 succeeded"));

    run_assert_cmd()
        .arg("mutants")
        .arg("--no-times")
        .current_dir(&tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .assert()
        .success()
        .stdout(contains("1 mutant tested: 1 caught"));
}

// Copyright 2021-2025 Martin Pool

//! Tests for cargo-mutants CLI layer.

use std::collections::HashSet;
use std::env;
use std::fs::{self, create_dir, create_dir_all, read_dir, read_to_string, rename, write, File};
use std::io::Write;
use std::path::Path;

use indoc::indoc;
use itertools::Itertools;
use jiff::Timestamp;
use predicate::str::{contains, is_match};
use predicates::prelude::*;
use pretty_assertions::assert_eq;

use regex::Regex;
use similar::TextDiff;
use tempfile::{tempdir, NamedTempFile, TempDir};

mod util;
use util::{copy_of_testdata, copy_testdata_to, run, OUTER_TIMEOUT};

use crate::util::outcome_json_counts;

#[test]
fn incorrect_cargo_subcommand() {
    // argv[1] "mutants" is missing here.
    run().arg("wibble").assert().code(1);
}

#[test]
fn missing_cargo_subcommand() {
    // argv[1] "mutants" is missing here.
    run().assert().code(1);
}

#[test]
fn option_in_place_of_cargo_subcommand() {
    // argv[1] "mutants" is missing here.
    run().args(["--list"]).assert().code(1);
}

#[test]
fn show_version() {
    run()
        .args(["mutants", "--version"])
        .assert()
        .success()
        .stdout(predicates::str::is_match(r"^cargo-mutants \d+\.\d+\.\d+(-.*)?\n$").unwrap());
}

#[test]
fn show_help() {
    // Asserting on the entire help message would be a bit too annoying to maintain.
    run()
        .args(["mutants", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "Usage: cargo mutants [OPTIONS] [-- <CARGO_TEST_ARGS>...]",
        ));
}

#[test]
fn emit_config_schema() {
    let output = run()
        .args(["mutants", "--emit-schema=config"])
        .assert()
        .success()
        .get_output()
        .clone();

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");

    // Parse as JSON to ensure it's valid
    let schema: serde_json::Value =
        serde_json::from_str(&stdout).expect("output should be valid JSON schema");

    // Verify it's a JSON schema with expected structure
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema",
    );
    assert_eq!(
        schema["$id"],
        "https://json.schemastore.org/cargo-mutants-config.json"
    );
    assert_eq!(schema["title"], "cargo-mutants configuration");
    assert_eq!(schema["type"], "object");

    // Verify some key properties exist
    let properties = schema["properties"]
        .as_object()
        .expect("schema should have properties object");

    assert!(properties.contains_key("output"));
    assert!(properties.contains_key("test_tool"));
    assert!(properties.contains_key("timeout_multiplier"));
    assert!(properties.contains_key("skip_calls"));

    // Verify the TestTool enum is properly defined
    let definitions = schema["$defs"]
        .as_object()
        .expect("schema should have definitions");
    assert!(definitions.contains_key("TestTool"));
}

#[test]
fn uses_cargo_env_var_to_run_cargo_so_invalid_value_fails() {
    let tmp_src_dir = copy_of_testdata("well_tested");
    let bogus_cargo = "NOTHING_NONEXISTENT_VOID";
    run()
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
fn tree_with_child_directories_is_well_tested() {
    let tmp_src_dir = copy_of_testdata("with_child_directories");
    run()
        .arg("mutants")
        .arg("-d")
        .arg(tmp_src_dir.path())
        .arg("-Ldebug")
        .assert()
        .success()
        .stderr(
            predicate::str::is_match(
                r#"DEBUG Copied source tree total_bytes=\d{3,} total_files=1[34]"#,
            )
            .unwrap(),
        );
    // The outcomes all have `diff_path` keys and they all identify files.
    let outcomes_json =
        read_to_string(tmp_src_dir.path().join("mutants.out/outcomes.json")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&outcomes_json).unwrap();
    let mut all_diffs = HashSet::new();
    for outcome_json in json["outcomes"].as_array().unwrap() {
        dbg!(&outcome_json);
        if outcome_json["scenario"].as_str() == Some("Baseline") {
            assert!(
                outcome_json
                    .get("diff_path")
                    .expect("has a diff_path")
                    .is_null(),
                "diff_path should be null"
            );
        } else {
            let diff_path = outcome_json["diff_path"].as_str().unwrap();
            let full_diff_path = tmp_src_dir.path().join("mutants.out").join(diff_path);
            assert!(full_diff_path.is_file(), "{diff_path:?} is not a file");
            assert!(all_diffs.insert(diff_path));
            let diff_content = read_to_string(&full_diff_path).expect("read diff file");
            assert!(
                diff_content.starts_with("--- src/"),
                "diff content in {full_diff_path:?} doesn't look right:\n{diff_content}"
            );
        }
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
    let test_start = Timestamp::now();
    let tmp_src_dir = copy_of_testdata("small_well_tested");
    run()
        .arg("mutants")
        .args(["--no-times", "--no-shuffle", "-v", "-V"])
        .current_dir(tmp_src_dir.path())
        .assert()
        .success()
        .stdout(predicate::function(|stdout: &str| {
            insta::assert_snapshot!(stdout);
            true
        }));
    // The log file should exist and include something that looks like a diff.
    let log_content = fs::read_to_string(
        tmp_src_dir
            .path()
            .join("mutants.out/log/src__lib.rs_line_5_col_5.log"),
    )
    .unwrap()
    .replace('\r', "");
    println!("log content:\n{log_content}");
    assert!(log_content.contains("*** mutation diff"));
    assert!(log_content.contains(indoc! { r#"
            *** mutation diff:
            --- src/lib.rs
            +++ replace factorial -> u32 with 0
            @@ -1,17 +1,13 @@
        "# }));
    assert!(log_content.contains(indoc! { r#"
             pub fn factorial(n: u32) -> u32 {
            -    let mut a = 1;
            -    for i in 2..=n {
            -        a *= i;
            -    }
            -    a
            +    0 /* ~ changed by cargo-mutants ~ */
             }
            "# }));
    // Also, it should contain output from the failed tests with mutations applied.
    assert!(log_content.contains("test test::test_factorial ... FAILED"));

    assert!(log_content.contains("---- test::test_factorial stdout ----"));
    assert!(log_content.contains("factorial(6) = 0"));

    let outcomes_json =
        read_to_string(tmp_src_dir.path().join("mutants.out/outcomes.json")).unwrap();
    let outcomes: serde_json::Value = outcomes_json.parse().unwrap();

    let start_time = &outcomes["start_time"];
    dbg!(&start_time);
    let start_time: Timestamp = start_time.as_str().unwrap().parse().unwrap();
    assert!(start_time >= test_start);

    let end_time = &outcomes["end_time"];
    dbg!(&end_time);
    let end_time: Timestamp = end_time.as_str().unwrap().parse().unwrap();
    assert!(end_time >= start_time);
    assert!(end_time <= Timestamp::now());

    // Verify cargo_mutants_version field exists
    let version = outcomes["cargo_mutants_version"]
        .as_str()
        .expect("cargo_mutants_version should be present and be a string");
    assert!(
        version.contains('.'),
        "cargo_mutants_version should look like a version"
    );
}

#[test]
fn test_small_well_tested_tree_with_baseline_skip() {
    let tmp_src_dir = copy_of_testdata("small_well_tested");
    run()
        .arg("mutants")
        .args(["--no-times", "--no-shuffle", "-v", "-V", "--baseline=skip"])
        .arg("-d")
        .arg(tmp_src_dir.path())
        .assert()
        .success()
        .stdout(predicate::function(|stdout: &str| {
            insta::assert_snapshot!(stdout);
            true
        }))
        .stderr(
            predicate::str::contains(
                "An explicit test timeout is recommended when using --baseline=skip",
            )
            .and(predicate::str::contains("Unmutated baseline in").not()),
        );
    assert!(!tmp_src_dir
        .path()
        .join("mutants.out/log/baseline.log")
        .exists());
}

#[test]
fn cdylib_tree_is_well_tested() {
    let tmp_src_dir = copy_of_testdata("cdylib");
    run()
        .arg("mutants")
        .args(["--no-times", "--no-shuffle", "-v", "-V"])
        .current_dir(tmp_src_dir.path())
        .assert()
        .success()
        .stdout(predicate::function(|stdout: &str| {
            insta::assert_snapshot!(stdout);
            true
        }));
}

#[test]
fn proc_macro_tree_is_well_tested() {
    let tmp_src_dir = copy_of_testdata("proc_macro");
    run()
        .arg("mutants")
        .args(["--no-times", "--no-shuffle", "-v", "-V"])
        .current_dir(tmp_src_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "2 mutants tested: 1 caught, 1 unviable",
        ));
}

#[test]
fn well_tested_tree_finds_no_problems() {
    let tmp_src_dir = copy_of_testdata("well_tested");
    run()
        .arg("mutants")
        .args(["--no-times", "--caught", "--unviable", "--no-shuffle"])
        .current_dir(tmp_src_dir.path())
        .assert()
        .success()
        .stdout(predicate::function(|stdout: &str| {
            insta::assert_snapshot!(stdout);
            true
        }));
    assert!(tmp_src_dir
        .path()
        .join("mutants.out/outcomes.json")
        .exists());
    let outcomes_json =
        fs::read_to_string(tmp_src_dir.path().join("mutants.out/outcomes.json")).unwrap();
    let outcomes: serde_json::Value = outcomes_json.parse().unwrap();
    let caught = outcomes["caught"]
        .as_i64()
        .expect("outcomes['caught'] is an integer");
    assert!(caught > 40, "expected more outcomes caught than {caught}");
    assert_eq!(outcomes["unviable"], 0);
    assert_eq!(outcomes["missed"], 0);
    assert_eq!(outcomes["timeout"], 0);
    assert_eq!(outcomes["total_mutants"], outcomes["caught"]);
    check_text_list_output(tmp_src_dir.path(), "well_tested_tree_finds_no_problems");
}

#[test]
fn well_tested_tree_check_only() {
    let tmp_src_dir = copy_of_testdata("well_tested");
    run()
        .args(["mutants", "--check", "--no-shuffle", "--no-times"])
        .current_dir(tmp_src_dir.path())
        .assert()
        .success()
        .stdout(predicate::function(|stdout: &str| {
            insta::assert_snapshot!(stdout);
            true
        }));
}

#[test]
fn well_tested_tree_check_only_shuffled() {
    let tmp_src_dir = copy_of_testdata("well_tested");
    run()
        .args(["mutants", "--check", "--no-times", "--shuffle"])
        .current_dir(tmp_src_dir.path())
        .assert()
        .success();
    // Caution: No assertions about output here, we just check that it runs.
}

#[test]
fn integration_test_source_is_not_mutated() {
    let tmp_src_dir = copy_of_testdata("integration_tests");
    run()
        .args(["mutants", "--no-times", "--no-shuffle", "--list-files"])
        .current_dir(tmp_src_dir.path())
        .assert()
        .success()
        .stdout("src/lib.rs\n");
    run()
        .args(["mutants", "--no-times", "--no-shuffle"])
        .current_dir(tmp_src_dir.path())
        .assert()
        .success();
    check_text_list_output(tmp_src_dir.path(), "integration_test_source_is_not_mutated");
}

#[test]
fn uncaught_mutant_in_factorial() {
    let tmp_src_dir = copy_of_testdata("factorial");

    run()
        .arg("mutants")
        .arg("--no-shuffle")
        .arg("--no-times")
        .arg("-d")
        .arg(tmp_src_dir.path())
        .assert()
        .code(2)
        .stderr("")
        .stdout(predicate::function(|stdout: &str| {
            insta::assert_snapshot!(stdout);
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

    check_text_list_output(tmp_src_dir.path(), "uncaught_mutant_in_factorial");
}

#[test]
fn factorial_mutants_with_all_logs() {
    // The log contains a lot of build output, which is hard to deal with, but let's check that
    // some key lines are there.
    let tmp_src_dir = copy_of_testdata("factorial");
    run()
        .arg("mutants")
        .arg("--all-logs")
        .arg("-v")
        .arg("-V")
        .arg("-d")
        .arg(tmp_src_dir.path())
        .assert()
        .code(2)
        .stderr(
            predicate::str::contains("WARN")
                .or(predicate::str::contains("ERR"))
                .not(),
        )
        .stdout(is_match(r"ok *Unmutated baseline in \d+s").unwrap())
        .stdout(
            is_match(r"MISSED *src/bin/factorial\.rs:\d+:\d+: replace main with \(\) in \d+s")
                .unwrap(),
        )
        .stdout(
            is_match(
                r"caught *src/bin/factorial\.rs:\d+:\d+: replace factorial -> u32 with 0 in \d+s",
            )
            .unwrap(),
        );
}

#[test]
fn factorial_mutants_with_all_logs_and_nocapture() {
    let tmp_src_dir = copy_of_testdata("factorial");
    run()
        .arg("mutants")
        .args(["--all-logs", "-v", "-V"])
        .arg("-d")
        .arg(tmp_src_dir.path())
        .args(["--", "--", "--nocapture"])
        .assert()
        .code(2)
        .stdout(contains("factorial(6) = 720")) // println from the test
        .stdout(contains("factorial(6) = 0")) // The mutated result
        ;
}

#[test]
fn factorial_mutants_no_copy_target() {
    let tmp_src_dir = copy_of_testdata("factorial");
    run()
        .arg("mutants")
        .args(["--no-times"])
        .arg("-d")
        .arg(tmp_src_dir.path())
        .assert()
        .code(2)
        .stderr("")
        .stdout(predicate::function(|stdout: &str| {
            insta::assert_snapshot!(stdout);
            true
        }));
}

#[test]
fn small_well_tested_mutants_with_cargo_arg_release() {
    let tmp_src_dir = copy_of_testdata("small_well_tested");
    run()
        .arg("mutants")
        .args(["--no-times", "--cargo-arg", "--release"])
        .arg("-d")
        .arg(tmp_src_dir.path())
        .assert()
        .success()
        .stderr("")
        .stdout(predicate::function(|stdout: &str| {
            insta::assert_snapshot!(stdout);
            true
        }));
    // Check that it was actually passed.
    let baseline_log_path = &tmp_src_dir.path().join("mutants.out/log/baseline.log");
    println!("{}", baseline_log_path.display());
    let log_content = fs::read_to_string(baseline_log_path).unwrap();
    println!("{log_content}");
    regex::Regex::new(r"cargo.* test --no-run --verbose .* --release")
        .unwrap()
        .captures(&log_content)
        .unwrap();
    regex::Regex::new(r"cargo.* test --verbose .* --release")
        .unwrap()
        .captures(&log_content)
        .unwrap();
}

#[test]
/// The `--output` directory creates the named directory if necessary, and then
/// creates `mutants.out` within it. `mutants.out` is not created in the
/// source directory in this case.
fn output_option() {
    let tmp_src_dir = copy_of_testdata("factorial");
    let output_tmpdir = TempDir::new().unwrap();
    let output_parent = output_tmpdir.path().join("output_parent");
    assert!(
        !tmp_src_dir.path().join("mutants.out").exists(),
        "mutants.out should not be in a clean copy of the test data"
    );
    run()
        .arg("mutants")
        .arg("--output")
        .arg(&output_parent)
        .args(["--check", "--no-times"])
        .arg("-d")
        .arg(tmp_src_dir.path())
        .assert()
        .success();
    assert!(
        !tmp_src_dir.path().join("mutants.out").exists(),
        "mutants.out should not be in the source directory after --output was given"
    );
    let mutants_out = output_parent.join("mutants.out");
    assert!(mutants_out.exists(), "mutants.out is in --output directory");
    for name in [
        "mutants.json",
        "debug.log",
        "outcomes.json",
        "missed.txt",
        "caught.txt",
        "timeout.txt",
        "unviable.txt",
    ] {
        assert!(mutants_out.join(name).is_file(), "{name} is in mutants.out",);
    }
}

#[test]
/// Set the `--output` directory via environment variable `CARGO_MUTANTS_OUTPUT`
fn output_option_use_env() {
    let tmp_src_dir = copy_of_testdata("factorial");
    let output_tmpdir = TempDir::new().unwrap();
    let output_via_env = output_tmpdir.path().join("output_via_env");
    assert!(
        !tmp_src_dir.path().join("mutants.out").exists(),
        "mutants.out should not be in a clean copy of the test data"
    );
    run()
        .env("CARGO_MUTANTS_OUTPUT", &output_via_env)
        .arg("mutants")
        .args(["--check", "--no-times"])
        .arg("-d")
        .arg(tmp_src_dir.path())
        .assert()
        .success();
    assert!(
        !tmp_src_dir.path().join("mutants.out").exists(),
        "mutants.out should not be in the source directory"
    );
    let mutants_out = output_via_env.join("mutants.out");
    assert!(
        mutants_out.exists(),
        "mutants.out is in $CARGO_MUTANTS_OUTPUT directory"
    );
    for name in [
        "mutants.json",
        "debug.log",
        "outcomes.json",
        "missed.txt",
        "caught.txt",
        "timeout.txt",
        "unviable.txt",
    ] {
        assert!(mutants_out.join(name).is_file(), "{name} is in mutants.out",);
    }
}

#[test]
fn already_failing_tests_are_detected_before_running_mutants() {
    let tmp_src_dir = copy_of_testdata("already_failing_tests");
    run()
        .arg("mutants")
        .current_dir(tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .assert()
        .code(4)
        .stdout(
            predicate::str::contains("running 1 test\ntest test_factorial ... FAILED")
                .normalize()
                .and(predicate::str::contains("assertion `left == right` failed"))
                .and(predicate::str::contains("72")) // the failing value should be in the output
                .and(predicate::str::contains("lib.rs:11:5"))
                .and(
                    predicate::str::contains("test result: FAILED. 0 passed; 1 failed;")
                        .normalize(),
                ),
        )
        .stderr(predicate::str::contains(
            "cargo test failed in an unmutated tree, so no mutants were tested",
        ));
}

#[test]
fn already_failing_doctests_are_detected() {
    let tmp_src_dir = copy_of_testdata("already_failing_doctests");
    run()
        .arg("mutants")
        .current_dir(tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .assert()
        .code(4) // CLEAN_TESTS_FAILED
        .stdout(contains("lib.rs - takes_one_arg (line 5) ... FAILED"))
        .stdout(contains(
            "this function takes 1 argument but 3 arguments were supplied",
        ))
        .stderr(predicate::str::contains(
            "cargo test failed in an unmutated tree, so no mutants were tested",
        ));
}

#[test]
fn already_failing_doctests_can_be_skipped_with_cargo_arg() {
    let tmp_src_dir = copy_of_testdata("already_failing_doctests");
    run()
        .arg("mutants")
        .args(["--", "--all-targets"])
        .current_dir(tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .assert()
        .code(0);
}

#[test]
fn source_tree_parse_fails() {
    let tmp_src_dir = copy_of_testdata("parse_fails");
    run()
        .arg("mutants")
        .current_dir(tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .assert()
        .failure() // TODO: This should be a distinct error code
        .stderr(contains("Error: failed to parse src/lib.rs"));
}

#[test]
fn source_tree_typecheck_fails() {
    let tmp_src_dir = copy_of_testdata("typecheck_fails");
    run()
        .arg("mutants")
        .current_dir(tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .assert()
        .failure() // TODO: This should be a distinct error code
        .stdout(is_match(r"FAILED *Unmutated baseline in \d+s").unwrap())
        .stdout(
            contains(r#""1" + 2 // Doesn't work in Rust: just as well!"#)
                .name("The problem source line"),
        )
        .stdout(contains("*** baseline"))
        .stdout(contains("test --no-run"))
        .stdout(contains("lib.rs:6"))
        .stdout(contains("*** result: "))
        .stderr(contains(
            "build failed in an unmutated tree, so no mutants were tested",
        ));
}

/// `CARGO_MUTANTS_MINIMUM_TEST_TIMEOUT` overrides the detected minimum timeout.
#[test]
fn minimum_test_timeout_from_env() {
    let tmp_src_dir = copy_of_testdata("small_well_tested");
    run()
        .arg("mutants")
        .env("CARGO_MUTANTS_MINIMUM_TEST_TIMEOUT", "1234")
        .current_dir(tmp_src_dir.path())
        .timeout(OUTER_TIMEOUT)
        .assert()
        .success()
        .stderr(predicate::str::contains("Auto-set test timeout to 1234s"));
}

/// In this tree, as the name suggests, tests will hang in a clean tree.
///
/// cargo-mutants should notice this when doing baseline tests and return a clean result.
///
/// We run the cargo-mutants subprocess with an enclosing timeout, so that the outer test will
/// fail rather than hang if cargo-mutants own timeout doesn't work as intended.
///
/// All these timeouts are a little brittle if the test machine is very slow.
#[test]
fn timeout_when_unmutated_tree_test_hangs() {
    let tmp_src_dir = copy_of_testdata("already_hangs");
    run()
        .arg("mutants")
        .args(["--timeout", "2.9"])
        .current_dir(tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .timeout(OUTER_TIMEOUT)
        .assert()
        .code(4) // exit_code::CLEAN_TESTS_FAILED
        .stdout(is_match(r"TIMEOUT *Unmutated baseline in \d+s").unwrap())
        .stderr(contains(
            "cargo test failed in an unmutated tree, so no mutants were tested",
        ));
}

/// Mutation of a const fn (evaluated at compile time) doesn't hang because
/// the compiler's built in check on evaluation time catches it.
#[test]
fn hang_const_fn_is_unviable() {
    let tmp_src_dir = copy_of_testdata("hang_const");
    let out = run()
        .arg("mutants")
        // no explicit timeouts
        .current_dir(tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .timeout(OUTER_TIMEOUT)
        .assert()
        .code(0);
    println!(
        "output:\n{}",
        String::from_utf8_lossy(&out.get_output().stdout)
    );
    let unviable_txt = read_to_string(tmp_src_dir.path().join("mutants.out/unviable.txt"))
        .expect("read unviable.txt");
    let caught_txt =
        read_to_string(tmp_src_dir.path().join("mutants.out/caught.txt")).expect("read caught.txt");
    let timeout_txt = read_to_string(tmp_src_dir.path().join("mutants.out/timeout.txt"))
        .expect("read timeout.txt");
    assert_eq!(
        unviable_txt,
        "src/lib.rs:2:5: replace should_stop_const -> bool with false\n"
    );
    assert_eq!(timeout_txt, "");
    assert_eq!(caught_txt, "");
    let outcomes_json: serde_json::Value =
        read_to_string(tmp_src_dir.path().join("mutants.out/outcomes.json"))
            .expect("read outcomes.json")
            .parse()
            .expect("parse outcomes.json");
    let outcomes = outcomes_json["outcomes"].as_array().unwrap();
    assert_eq!(outcomes.len(), 2, "should have one baseline and one mutant");
    assert_eq!(
        outcomes[1]["scenario"]["Mutant"]["function"]["function_name"],
        "should_stop_const"
    );
    assert_eq!(outcomes[1]["summary"], "Unviable");
    assert_eq!(outcomes_json["timeout"], 0);

    // The problem should be detected by the build phase without running tests.
    let phases_for_const_fn = outcomes[1]["phase_results"].as_array().unwrap();
    assert_eq!(phases_for_const_fn.len(), 1);
    assert_eq!(phases_for_const_fn[0]["phase"], "Build");
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
fn mutants_causing_tests_to_hang_are_stopped_by_manual_timeout() {
    let tmp_src_dir = copy_of_testdata("hang_when_mutated");
    // Also test that it accepts decimal seconds
    let out = run()
        .arg("mutants")
        .args(["-t", "8.1", "--build-timeout=15.5"])
        .current_dir(tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .timeout(OUTER_TIMEOUT)
        .assert()
        .code(3); // exit_code::TIMEOUT
    println!(
        "output:\n{}",
        String::from_utf8_lossy(&out.get_output().stdout)
    );
    let unviable_txt = read_to_string(tmp_src_dir.path().join("mutants.out/unviable.txt"))
        .expect("read unviable.txt");
    let caught_txt =
        read_to_string(tmp_src_dir.path().join("mutants.out/caught.txt")).expect("read caught.txt");
    let timeout_txt = read_to_string(tmp_src_dir.path().join("mutants.out/timeout.txt"))
        .expect("read timeout.txt");
    assert!(
        timeout_txt.contains("replace should_stop -> bool with false"),
        "expected text not found in:\n{timeout_txt}"
    );
    assert_eq!(unviable_txt, "", "expected text not found in unviable.txt");
    assert!(
        caught_txt.contains("replace should_stop -> bool with true"),
        "expected text not found in:\n{caught_txt}"
    );
    assert!(
        caught_txt.contains("replace controlled_loop -> usize with 0"),
        "expected text not found in:\n{caught_txt}"
    );
    let outcomes_json: serde_json::Value =
        read_to_string(tmp_src_dir.path().join("mutants.out/outcomes.json"))
            .expect("read outcomes.json")
            .parse()
            .expect("parse outcomes.json");
    assert_eq!(outcomes_json["timeout"], 1);
}

/// If you set `--cap-lints` to `true`, then the compiler's quasi-lint on excessive
/// runtime from a const fn won't protect it from a hang. However, the explicit
/// build timeout will still catch it.
#[test]
fn hang_avoided_by_build_timeout_with_cap_lints() {
    let tmp_src_dir = copy_of_testdata("hang_const");
    let out = run()
        .arg("mutants")
        .args(["--build-timeout=10", "--regex=const", "--cap-lints=true"])
        .current_dir(tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .timeout(OUTER_TIMEOUT)
        .assert();
    println!(
        "debug log:\n===\n{}\n===",
        read_to_string(tmp_src_dir.path().join("mutants.out/debug.log")).unwrap_or_default()
    );
    out.code(3); // exit_code::TIMEOUT
    let timeout_txt = read_to_string(tmp_src_dir.path().join("mutants.out/timeout.txt"))
        .expect("read timeout.txt");
    assert_eq!(
        timeout_txt, "src/lib.rs:2:5: replace should_stop_const -> bool with false\n",
        "expected text not found in timeout.txt"
    );
}

#[test]
fn constfn_mutation_passes_check() {
    let tmp_src_dir = copy_of_testdata("hang_when_mutated");
    let cmd = run()
        .arg("mutants")
        .args(["--check", "--build-timeout=10"])
        .current_dir(tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .timeout(OUTER_TIMEOUT)
        .assert()
        .code(0);
    println!("{}", String::from_utf8_lossy(&cmd.get_output().stdout));
}

#[test]
fn log_file_names_are_short_and_dont_collide() {
    // The "well_tested" tree can generate multiple mutants from single lines. They get distinct file names.
    let tmp_src_dir = copy_of_testdata("well_tested");
    let cmd_assert = run()
        .arg("mutants")
        .args(["--check", "-v", "-V"])
        .current_dir(tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .assert()
        .success();
    println!(
        "{}",
        String::from_utf8_lossy(&cmd_assert.get_output().stdout)
    );
    let log_dir = tmp_src_dir.path().join("mutants.out").join("log");
    let all_log_names = read_dir(log_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_str().unwrap().to_string())
        .inspect(|filename| println!("{filename}"))
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

fn setup_relative_dependency(tree_name: &str) -> TempDir {
    let tmp = TempDir::new().unwrap();
    let tmp_path = tmp.path();
    let tmp_testdata = tmp_path.join("testdata");
    create_dir(&tmp_testdata).unwrap();
    copy_testdata_to(tree_name, tmp_testdata.join(tree_name));

    // Make a tiny version of the 'mutants' crate so that it can be imported by a relative
    // dependency or otherwise.
    //
    // This is a bit annoying because
    // - the dependency must be published to crates.io to be a dependency that's overridden
    // - but, we have a copy of it in this tree so we can override the dependency, without
    //   needing to download it during the tests
    copy_testdata_to("mutants_attrs", tmp_path.join("mutants_attrs"));
    tmp
}

#[test]
fn cargo_mutants_in_override_dependency_tree_passes() {
    let tree_name = "override_dependency";
    let tmp = setup_relative_dependency(tree_name);
    run()
        .arg("mutants")
        .arg("--no-times")
        .arg("--no-shuffle")
        .arg("-d")
        .arg(tmp.path().join("testdata").join(tree_name))
        .assert()
        .success()
        .stdout(predicate::function(|stdout: &str| {
            insta::assert_snapshot!(stdout);
            true
        }));
}

#[test]
fn cargo_mutants_in_relative_dependency_tree_passes() {
    let tree_name = "relative_dependency";
    let tmp = setup_relative_dependency(tree_name);
    copy_testdata_to("dependency", tmp.path().join("testdata").join("dependency"));
    run()
        .arg("mutants")
        .arg("--no-times")
        .arg("--no-shuffle")
        .arg("-d")
        .arg(tmp.path().join("testdata").join(tree_name))
        .assert()
        .success()
        .stdout(predicate::function(|stdout: &str| {
            insta::assert_snapshot!(stdout);
            true
        }));
}

#[test]
fn cargo_mutants_in_replace_dependency_tree_passes() {
    let tree_name = "replace_dependency";
    let tmp = setup_relative_dependency(tree_name);
    run()
        .arg("mutants")
        .arg("--no-times")
        .arg("--no-shuffle")
        .arg("-d")
        .arg(tmp.path().join("testdata").join(tree_name))
        .assert()
        .success()
        .stdout(predicate::function(|stdout: &str| {
            insta::assert_snapshot!(stdout);
            true
        }));
}

#[test]
fn cargo_mutants_in_patch_dependency_tree_passes() {
    let tmp = setup_relative_dependency("patch_dependency");
    run()
        .arg("mutants")
        .arg("--no-times")
        .arg("--no-shuffle")
        .arg("-d")
        .arg(tmp.path().join("testdata").join("patch_dependency"))
        .assert()
        .success()
        .stdout(predicate::function(|stdout: &str| {
            insta::assert_snapshot!(stdout);
            true
        }));
}

/// This test would fail if mutants aren't correctly removed from the tree after
/// testing, which would cause all later mutants to be incorrectly marked as
/// caught.
///
/// This was suggested by `Mutant::unapply` being marked as missed.
#[test]
fn mutants_are_unapplied_after_testing_so_later_missed_mutants_are_found() {
    // This needs --no-shuffle because failure to unapply will show up when the
    // uncaught mutant is not the first file tested.
    let tmp_src_dir = copy_of_testdata("unapply");
    run()
        .args(["mutants", "--no-times", "--no-shuffle"])
        .arg("-d")
        .arg(tmp_src_dir.path())
        .assert()
        .code(2) // some were missed
        .stdout(predicate::function(|stdout: &str| {
            insta::assert_snapshot!(stdout);
            true
        }));
}

#[test]
fn strict_warnings_about_unused_variables_are_disabled_so_mutants_compile() {
    let tmp_src_dir = copy_of_testdata("strict_warnings");
    run()
        .arg("mutants")
        .arg("--check")
        .arg("--no-times")
        .current_dir(tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .assert()
        .success();

    run()
        .arg("mutants")
        .arg("--no-times")
        .current_dir(tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .assert()
        .success();
}

fn check_text_list_output(dir: &Path, test_name: &str) {
    // There is a `missed.txt` file with the right content, etc.
    for name in ["missed", "caught", "timeout", "unviable"] {
        let path = dir.join(format!("mutants.out/{name}.txt"));
        let content = fs::read_to_string(&path).unwrap();
        insta::assert_snapshot!(format!("{test_name}__{name}.txt"), content);
    }
}

/// `cargo mutants --completions SHELL` produces a shell script for some
/// well-known shells.
///
/// We won't check the content but let's just make sure that it succeeds
/// and produces some non-empty output.
#[test]
fn completions_option_generates_something() {
    for shell in ["bash", "fish", "zsh", "powershell"] {
        println!("completions for {shell}");
        run()
            .arg("mutants")
            .arg("--completions")
            .arg(shell)
            .assert()
            .success()
            .stdout(predicate::str::is_empty().not());
    }
}

#[test]
fn config_option_reads_custom_file() {
    let tmp_src_dir = copy_of_testdata("well_tested");

    // Create a custom config file with specific error values
    let custom_config_path = tmp_src_dir.path().join("custom_config.toml");
    fs::write(
        &custom_config_path,
        r#"error_values = ["anyhow::anyhow!(\"custom test error\")", "std::io::Error::new(std::io::ErrorKind::Other, \"custom\")"]
additional_cargo_args = ["--verbose"]
timeout_multiplier = 2.5
"#,
    )
    .unwrap();

    // Test that --config reads the custom file
    let output = run()
        .args(["mutants", "--config"])
        .arg(&custom_config_path)
        .args(["--list", "-d"])
        .arg(tmp_src_dir.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8_lossy(&output);

    // Should contain the custom error value from our config file
    assert!(
        output_str.contains("custom test error"),
        "Output should contain custom error value from config file"
    );
}

#[test]
fn config_option_mutual_exclusion_with_no_config() {
    let tmp_src_dir = copy_of_testdata("well_tested");
    let custom_config_path = tmp_src_dir.path().join("test_config.toml");
    fs::write(&custom_config_path, "timeout_multiplier = 2.0").unwrap();

    // Test that --config and --no-config are mutually exclusive
    run()
        .args(["mutants", "--config"])
        .arg(&custom_config_path)
        .args(["--no-config", "--list", "-d"])
        .arg(tmp_src_dir.path())
        .assert()
        .failure()
        .stderr(contains("cannot be used with"));
}

#[test]
fn config_option_nonexistent_file() {
    let tmp_src_dir = copy_of_testdata("well_tested");

    // Test error handling for non-existent config file
    run()
        .args(["mutants", "--config", "/nonexistent/config.toml"])
        .args(["--list", "-d"])
        .arg(tmp_src_dir.path())
        .assert()
        .failure()
        .stderr(
            contains("read config /nonexistent/config.toml").and(
                contains("No such file or directory")
                    .or(contains("The system cannot find the path specified")),
            ),
        );
}

#[test]
fn config_option_vs_default_behavior() {
    let tmp_src_dir = copy_of_testdata("well_tested");

    // Create default .cargo/mutants.toml config
    create_dir(tmp_src_dir.path().join(".cargo")).unwrap();
    fs::write(
        tmp_src_dir.path().join(".cargo/mutants.toml"),
        r#"error_values = ["anyhow::anyhow!(\"default error\")"]"#,
    )
    .unwrap();

    // Create custom config file
    let custom_config_path = tmp_src_dir.path().join("custom.toml");
    fs::write(
        &custom_config_path,
        r#"error_values = ["anyhow::anyhow!(\"custom error\")"]"#,
    )
    .unwrap();

    // Test default behavior (should use .cargo/mutants.toml)
    let default_output = run()
        .args(["mutants", "--list", "-d"])
        .arg(tmp_src_dir.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let default_str = String::from_utf8_lossy(&default_output);
    assert!(
        default_str.contains("default error"),
        "Default behavior should use .cargo/mutants.toml"
    );

    // Test custom config file (should use custom.toml)
    let custom_output = run()
        .args(["mutants", "--config"])
        .arg(&custom_config_path)
        .args(["--list", "-d"])
        .arg(tmp_src_dir.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let custom_str = String::from_utf8_lossy(&custom_output);
    assert!(
        custom_str.contains("custom error"),
        "Custom config should override default config"
    );
    assert!(
        !custom_str.contains("default error"),
        "Custom config should not contain default error values"
    );

    // Test --no-config (should use neither)
    let no_config_output = run()
        .args(["mutants", "--no-config", "--list", "-d"])
        .arg(tmp_src_dir.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let no_config_str = String::from_utf8_lossy(&no_config_output);
    assert!(
        !no_config_str.contains("default error"),
        "--no-config should not use default config"
    );
    assert!(
        !no_config_str.contains("custom error"),
        "--no-config should not use custom config"
    );
}

#[test]
fn example_config_file_can_be_loaded() {
    let tmp_src_dir = copy_of_testdata("well_tested");

    // Test that the example config file in examples/ directory can be loaded successfully
    run()
        .args(["mutants", "--config", "examples/custom_config.toml"])
        .args(["--list", "-d"])
        .arg(tmp_src_dir.path())
        .assert()
        .success()
        .stdout(contains("custom mutant error"));
}

#[test]
fn test_with_nextest_on_small_tree() {
    let tmp_src_dir = copy_of_testdata("small_well_tested");
    let assert = run()
        .args(["mutants", "--test-tool", "nextest", "-vV", "--no-shuffle"])
        .arg("-d")
        .arg(tmp_src_dir.path())
        .assert()
        .stderr(predicates::str::contains("WARN").not())
        .stdout(
            predicates::str::contains("4 mutants tested")
                .and(predicates::str::contains("Found 4 mutants to test"))
                .and(predicates::str::contains("4 caught")),
        )
        .success();
    println!(
        "stdout:\n{}",
        String::from_utf8_lossy(&assert.get_output().stdout)
    );
}

#[test]
fn unexpected_nextest_error_code_causes_a_warning() {
    let temp = TempDir::new().unwrap();
    let path = temp.path();
    write(
        path.join("Cargo.toml"),
        r#"[package]
            name = "cargo-mutants-test"
            version = "0.1.0"
            publish = false
            "#,
    )
    .unwrap();
    create_dir(path.join("src")).unwrap();
    write(
        path.join("src/main.rs"),
        r#"fn main() {
        println!("{}", 1 + 2);
        }"#,
    )
    .unwrap();
    create_dir(path.join(".config")).unwrap();
    write(
        path.join(".config/nextest.toml"),
        r#"nextest-version = { required = "9999.0.0" }"#,
    )
    .unwrap();

    let assert = run()
        .args([
            "mutants",
            "--test-tool",
            "nextest",
            "-vV",
            "--no-shuffle",
            "-Ldebug",
        ])
        .arg("-d")
        .arg(temp.path())
        .assert()
        .stderr(predicates::str::contains(
            "nextest process exited with unexpected code (allowed: [4, 100, 101]) code=92",
        ))
        .code(4); // CLEAN_TESTS_FAILED
    println!(
        "stdout:\n{}",
        String::from_utf8_lossy(&assert.get_output().stdout)
    );
    println!(
        "stderr:\n{}",
        String::from_utf8_lossy(&assert.get_output().stderr)
    );
}

#[test]
fn gitignore_respected_when_enabled() {
    // Make a tree with a (dumb) gitignore that excludes the source file; when you copy it
    // to a build directory with gitignore enabled, the source file should not be there and so the check will fail.
    let tmp = copy_of_testdata("factorial");
    // There must be something that looks like a `.git` dir, otherwise we don't read
    // `.gitignore` files.
    create_dir(tmp.path().join(".git")).unwrap();
    write(tmp.path().join(".gitignore"), b"src\n").unwrap();
    run()
        .args(["mutants", "--check", "--gitignore=true", "-d"])
        .arg(tmp.path())
        .assert()
        .stdout(predicates::str::contains("can't find `factorial` bin"))
        .code(4);
}

#[test]
fn gitignore_can_be_turned_off() {
    // Make a tree with a (dumb) gitignore that excludes the source file; when you copy it
    // to a build directory, with gitignore off, it succeeds.
    let tmp = copy_of_testdata("factorial");
    write(tmp.path().join(".gitignore"), b"src\n").unwrap();
    run()
        .args(["mutants", "--check", "--gitignore=false", "-d"])
        .arg(tmp.path())
        .assert()
        .success();
}

#[test]
fn gitignore_not_respected_by_default() {
    // Make a tree with a (dumb) gitignore that excludes the source file; when you copy it
    // to a build directory by default (gitignore=false), it should succeed because gitignore is ignored.
    let tmp = copy_of_testdata("factorial");
    // There must be something that looks like a `.git` dir, otherwise we don't read
    // `.gitignore` files anyway.
    create_dir(tmp.path().join(".git")).unwrap();
    write(tmp.path().join(".gitignore"), b"src\n").unwrap();
    run()
        .args(["mutants", "--check", "-d"])
        .arg(tmp.path())
        .assert()
        .success();
}

/// A tree containing a symlink that must exist for the tests to pass works properly.
#[test]
fn symlink_in_source_tree_is_copied() {
    let tmp = copy_of_testdata("symlink");
    let testdata = tmp.path().join("testdata");
    #[cfg(unix)]
    std::os::unix::fs::symlink("target", testdata.join("symlink")).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file("target", testdata.join("symlink")).unwrap();
    assert!(tmp.path().join("testdata").join("symlink").is_symlink());
    run()
        .args(["mutants", "-d"])
        .arg(tmp.path())
        .assert()
        .success();
}

/// Only on Windows, backslash can be used as a path separator in filters.
#[cfg(windows)]
#[test]
fn list_mutants_well_tested_exclude_folder_containing_backslash_on_windows() {
    // This could be written more simply as `--exclude module` but we want to
    // test that backslash is accepted.
    let tmp = copy_of_testdata("with_child_directories");
    run()
        .arg("mutants")
        .args(["--list", "--exclude", "**\\module\\**\\*.rs"])
        .current_dir(tmp.path())
        .assert()
        .stdout(
            predicates::str::contains(r"src/module")
                .not()
                .and(predicates::str::contains(r"src/methods.rs")),
        );
}

/// If the test hangs and the user (in this case the test suite) interrupts it, then
/// the `cargo test` child should be killed.
///
/// This is a bit hard to directly observe: the property that we really most care
/// about is that _all_ grandchild processes are also killed and nothing is left
/// behind. (On Unix, this is accomplished by use of a pgroup.) However that's a bit
/// hard to mechanically check without reading and interpreting the process tree, which
/// seems likely to be a bit annoying to do portably and without flakes.
/// (But maybe we still should?)
///
/// An easier thing to test is that the cargo-mutants process _thinks_ it has killed
/// the children, and we can observe this in the debug log.
///
/// In this test cargo-mutants has a very long timeout, but the test driver has a
/// short timeout, so it should kill cargo-mutants.
// TODO: An equivalent test on Windows?
#[cfg(unix)]
#[test]
fn interrupt_caught_and_kills_children() {
    // Test a tree that has enough tests that we'll probably kill it before it completes.

    use std::process::{Command, Stdio};
    use std::thread::sleep;
    use std::time::Duration;

    use nix::libc::pid_t;
    use nix::sys::signal::{kill, SIGTERM};
    use nix::unistd::Pid;

    use crate::util::MAIN_BINARY;

    let tmp_src_dir = copy_of_testdata("well_tested");
    // We can't use `assert_cmd` `timeout` here because that sends the child a `SIGKILL`,
    // which doesn't give it a chance to clean up. And, `std::process::Command` only
    // has an abrupt kill.

    // Drop RUST_BACKTRACE because the traceback mentions "panic" handler functions
    // and we want to check that the process does not panic.

    // Skip baseline because firstly it should already pass but more importantly
    // #333 exhibited only during non-baseline scenarios.
    let args = [
        MAIN_BINARY.to_str().unwrap(),
        "mutants",
        "--timeout=300",
        "--baseline=skip",
        "--level=trace",
    ];

    println!("Running: {args:?}");
    let mut child = Command::new(args[0])
        .args(&args[1..])
        .current_dir(&tmp_src_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove("RUST_BACKTRACE")
        .spawn()
        .expect("spawn child");

    sleep(Duration::from_secs(2)); // Let it get started
    assert!(
        child.try_wait().expect("try to wait for child").is_none(),
        "child exited early"
    );

    println!("Sending SIGTERM to cargo-mutants...");
    kill(Pid::from_raw(child.id() as pid_t), SIGTERM).expect("send SIGTERM");

    println!("Wait for cargo-mutants to exit...");
    let output = child
        .wait_with_output()
        .expect("wait for child after SIGTERM");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("stdout:\n{stdout}");
    println!("stderr:\n{stderr}");

    assert!(stderr.contains("interrupted"));
    // We used to look here for some other trace messages about how it's interrupted, but
    // that seems to be racy: sometimes the parent sees the child interrupted before it
    // emits these messages? Anyhow, it's not essential.

    // This shouldn't cause a panic though (#333)
    assert!(!stderr.contains("panic"));
    // And we don't want duplicate messages about workers failing.
    assert!(!stderr.contains("Worker thread failed"));
}

#[test]
fn env_var_controls_trace() {
    let tmp = copy_of_testdata("never_type");
    run()
        .env("CARGO_MUTANTS_TRACE_LEVEL", "trace")
        .args(["mutants", "--list"])
        .arg("-d")
        .arg(tmp.path())
        .assert()
        // This is a debug!() message; it should only be seen if the trace var
        // was wired correctly to stderr.
        .stderr(predicate::str::contains(
            "No mutants generated for this return type",
        ));
}

#[test]
fn shard_divides_all_mutants() {
    // For speed, this only lists the mutants, trusting that the mutants
    // that are listed are the ones that are run.
    let tmp = copy_of_testdata("well_tested");
    let common_args = ["mutants", "--list", "-d", tmp.path().to_str().unwrap()];
    let full_list = String::from_utf8(
        run()
            .args(common_args)
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .unwrap()
    .lines()
    .map(ToOwned::to_owned)
    .collect_vec();

    let n_shards = 5;
    let rr_shard_lists = (0..n_shards)
        .map(|k| {
            String::from_utf8(
                run()
                    .args(common_args)
                    .args([
                        "--shard",
                        &format!("{k}/{n_shards}"),
                        "--sharding=round-robin",
                    ])
                    .assert()
                    .success()
                    .get_output()
                    .stdout
                    .clone(),
            )
            .unwrap()
            .lines()
            .map(ToOwned::to_owned)
            .collect_vec()
        })
        .collect_vec();

    // If you combine all the mutants selected for each shard, you get the
    // full list, with nothing lost or duplicated, disregarding order.
    //
    // If we had a bug where we shuffled before sharding, then the shards would
    // see inconsistent lists and this test would fail in at least some attempts.
    assert_eq!(
        rr_shard_lists.iter().flatten().sorted().collect_vec(),
        full_list.iter().sorted().collect_vec()
    );

    // The shards should also be approximately the same size.
    let shard_lens = rr_shard_lists.iter().map(|l| l.len()).collect_vec();
    assert!(shard_lens.iter().max().unwrap() - shard_lens.iter().min().unwrap() <= 1);

    // And the shards are disjoint
    for i in 0..n_shards {
        for j in 0..n_shards {
            if i != j {
                assert!(
                    rr_shard_lists[i]
                        .iter()
                        .all(|m| !rr_shard_lists[j].contains(m)),
                    "shard {} contains {}",
                    j,
                    rr_shard_lists[j]
                        .iter()
                        .filter(|m| rr_shard_lists[j].contains(m))
                        .join(", ")
                );
            }
        }
    }

    // If you reassemble the round-robin shards in order, you get the original order back.
    //
    // To do so: cycle around the list of shards, taking one from each shard in order, until
    // we get to the end of any list.
    let mut reassembled = Vec::new();
    let mut rr_iters = rr_shard_lists
        .clone()
        .into_iter()
        .map(|l| l.into_iter())
        .collect_vec();
    let mut i = 0;
    let mut limit = 0;
    for name in rr_iters[i].by_ref() {
        reassembled.push(name);
        i = (i + 1) % n_shards;
        limit += 1;
        assert!(limit < full_list.len() * 2, "too many iterations");
    }

    // Check with slice sharding, the new default
    let slice_shard_lists = (0..n_shards)
        .map(|k| {
            String::from_utf8(
                run()
                    .args(common_args)
                    .args([&format!("--shard={k}/{n_shards}")]) //  "--sharding=slice"
                    .assert()
                    .success()
                    .get_output()
                    .stdout
                    .clone(),
            )
            .unwrap()
            .lines()
            .map(ToOwned::to_owned)
            .collect_vec()
        })
        .collect_vec();

    // These can just be concatenated
    let slice_reassembled = slice_shard_lists.into_iter().flatten().collect_vec();
    assert_eq!(slice_reassembled, full_list);
}

/// Test that `--jobs` seems to launch multiple threads.
///
/// It's a bit hard to assess that multiple jobs really ran in parallel,
/// but we can at least check that the option is accepted, and see that the
/// debug log looks like it's using multiple threads.
#[test]
fn jobs_option_accepted_and_causes_multiple_threads() {
    let testdata = copy_of_testdata("small_well_tested");
    run()
        .arg("mutants")
        .arg("-d")
        .arg(testdata.path())
        .arg("-j2")
        .arg("--minimum-test-timeout=120") // to avoid flakes on slow CI
        .assert()
        .success();
    let debug_log =
        read_to_string(testdata.path().join("mutants.out/debug.log")).expect("read debug log");
    println!("debug log:\n{debug_log}");
    // This might be brittle, as the ThreadId debug form is not specified, and
    // also _possibly_ everything will complete on one thread before the next
    // gets going, though that seems unlikely.
    let re = Regex::new(r#"start thread thread_id=ThreadId\(\d+\)"#).expect("compile regex");
    let matches = re
        .find_iter(&debug_log)
        .map(|m| m.as_str())
        .unique()
        .collect::<Vec<_>>();
    println!("threadid matches: {matches:?}");
    assert!(
        matches.len() > 1,
        "expected more than {} thread ids in debug log",
        matches.len()
    );
}

#[test]
fn warn_about_too_many_jobs() {
    let testdata = copy_of_testdata("small_well_tested");
    run()
        .arg("mutants")
        .arg("-d")
        .arg(testdata.path())
        .arg("-j40")
        .arg("--shard=0/1000")
        .assert()
        .stderr(predicates::str::contains(
            "WARN --jobs=40 is probably too high",
        ))
        .success();
}

#[test]
#[allow(clippy::too_many_lines)] // long but pretty straightforward
fn iterate_retries_missed_mutants() {
    let temp = tempdir().unwrap();

    write(
        temp.path().join("Cargo.toml"),
        indoc! { r#"
            [package]
            name = "cargo_mutants_iterate"
            edition = "2021"
            version = "0.0.0"
            publish = false
        "# },
    )
    .unwrap();
    create_dir(temp.path().join("src")).unwrap();
    create_dir(temp.path().join("tests")).unwrap();

    // First, write some untested code, and expect that the mutant is missed.
    write(
        temp.path().join("src/lib.rs"),
        indoc! { r#"
            pub fn is_two(a: usize) -> bool { a == 2 }
        "#},
    )
    .unwrap();

    run()
        .arg("mutants")
        .arg("-d")
        .arg(temp.path())
        .arg("--list")
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(indoc! { r#"
            src/lib.rs:1:35: replace is_two -> bool with true
            src/lib.rs:1:35: replace is_two -> bool with false
            src/lib.rs:1:37: replace == with != in is_two
        "# });

    run()
        .arg("mutants")
        .arg("--no-shuffle")
        .arg("-d")
        .arg(temp.path())
        .assert()
        .code(2); // missed mutants

    assert_eq!(
        read_to_string(temp.path().join("mutants.out/missed.txt")).unwrap(),
        indoc! { r#"
            src/lib.rs:1:35: replace is_two -> bool with true
            src/lib.rs:1:35: replace is_two -> bool with false
            src/lib.rs:1:37: replace == with != in is_two
        "# }
    );
    assert_eq!(
        read_to_string(temp.path().join("mutants.out/caught.txt")).unwrap(),
        ""
    );
    assert!(!temp
        .path()
        .join("mutants.out/previously_caught.txt")
        .is_file());

    // Now add a test that should catch this.
    write(
        temp.path().join("tests/main.rs"),
        indoc! { r#"
        use cargo_mutants_iterate::*;

        #[test]
        fn some_test() {
            assert!(is_two(2));
            assert!(!is_two(4));
        }
    "#},
    )
    .unwrap();

    run()
        .arg("mutants")
        .arg("--no-shuffle")
        .arg("-d")
        .arg(temp.path())
        .assert()
        .code(0); // caught it

    assert_eq!(
        read_to_string(temp.path().join("mutants.out/missed.txt")).unwrap(),
        ""
    );
    assert_eq!(
        read_to_string(temp.path().join("mutants.out/caught.txt")).unwrap(),
        indoc! { r#"
            src/lib.rs:1:35: replace is_two -> bool with true
            src/lib.rs:1:35: replace is_two -> bool with false
            src/lib.rs:1:37: replace == with != in is_two
        "# }
    );

    // Now that everything's caught, run tests again and there should be nothing to test,
    // on both the first and second run with --iterate
    for _ in 0..2 {
        run()
            .arg("mutants")
            .args(["--list", "--iterate"])
            .arg("-d")
            .arg(temp.path())
            .assert()
            .success()
            .stdout("");
        run()
            .arg("mutants")
            .args(["--no-shuffle", "--iterate", "--in-place"])
            .arg("-d")
            .arg(temp.path())
            .assert()
            .success()
            .stderr(predicate::str::contains(
                "No mutants found under the active filters",
            ))
            .stdout(predicate::str::contains("Found 0 mutants to test"));
        assert_eq!(
            read_to_string(temp.path().join("mutants.out/caught.txt")).unwrap(),
            ""
        );
        assert_eq!(
            read_to_string(temp.path().join("mutants.out/previously_caught.txt"))
                .unwrap()
                .lines()
                .count(),
            3
        );
    }

    // Add some more code and it should be seen as untested.
    let mut src = File::options()
        .append(true)
        .open(temp.path().join("src/lib.rs"))
        .unwrap();
    src.write_all("pub fn not_two(a: usize) -> bool { !is_two(a) }\n".as_bytes())
        .unwrap();
    drop(src);

    // We should see only the new function as untested
    let added_mutants = indoc! { r#"
        src/lib.rs:2:36: replace not_two -> bool with true
        src/lib.rs:2:36: replace not_two -> bool with false
        src/lib.rs:2:36: delete ! in not_two
    "# };
    run()
        .arg("mutants")
        .args(["--list", "--iterate"])
        .arg("-d")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(added_mutants);

    // These are missed by a new incremental run
    run()
        .arg("mutants")
        .args(["--no-shuffle", "--iterate", "--in-place"])
        .arg("-d")
        .arg(temp.path())
        .assert()
        .code(2);
    assert_eq!(
        read_to_string(temp.path().join("mutants.out/missed.txt")).unwrap(),
        added_mutants
    );

    // Add a new test that catches some but not all mutants
    File::options()
        .append(true)
        .open(temp.path().join("tests/main.rs"))
        .unwrap()
        .write_all("#[test] fn three_is_not_two() { assert!(not_two(3)); }\n".as_bytes())
        .unwrap();
    run()
        .arg("mutants")
        .args(["--no-shuffle", "--iterate", "--in-place"])
        .arg("-d")
        .arg(temp.path())
        .assert()
        .code(2);
    assert_eq!(
        read_to_string(temp.path().join("mutants.out/missed.txt")).unwrap(),
        "src/lib.rs:2:36: replace not_two -> bool with true\n"
    );

    // There should only be one more mutant to test
    run()
        .arg("mutants")
        .args(["--list", "--iterate", "--in-place"])
        .arg("-d")
        .arg(temp.path())
        .assert()
        .success()
        .stdout("src/lib.rs:2:36: replace not_two -> bool with true\n");

    // Add another test
    File::options()
        .append(true)
        .open(temp.path().join("tests/main.rs"))
        .unwrap()
        .write_all("#[test] fn two_is_not_not_two() { assert!(!not_two(2)); }\n".as_bytes())
        .unwrap();
    run()
        .arg("mutants")
        .args(["--list", "--iterate", "--in-place"])
        .arg("-d")
        .arg(temp.path())
        .assert()
        .success()
        .stdout("src/lib.rs:2:36: replace not_two -> bool with true\n");
    run()
        .arg("mutants")
        .args(["--no-shuffle", "--iterate", "--in-place"])
        .arg("-d")
        .arg(temp.path())
        .assert()
        .success();
    assert_eq!(
        read_to_string(temp.path().join("mutants.out/missed.txt")).unwrap(),
        ""
    );
    assert_eq!(
        read_to_string(temp.path().join("mutants.out/caught.txt")).unwrap(),
        "src/lib.rs:2:36: replace not_two -> bool with true\n"
    );
    assert_eq!(
        read_to_string(temp.path().join("mutants.out/previously_caught.txt"))
            .unwrap()
            .lines()
            .count(),
        5
    );

    // nothing more is missed
    run()
        .arg("mutants")
        .args(["--list", "--iterate", "--in-place"])
        .arg("-d")
        .arg(temp.path())
        .assert()
        .success()
        .stdout("");
}

/// `INSTA_UPDATE=always` in the environment will cause Insta to update
/// the snaphots, so the tests will pass, so mutants will not be caught.
/// This test checks that cargo-mutants sets the environment variable
/// so that mutants are caught properly.
#[test]
fn insta_test_failures_are_detected() {
    for insta_update in ["auto", "always"] {
        println!("INSTA_UPDATE={insta_update}");
        let tmp_src_dir = copy_of_testdata("insta");
        run()
            .arg("mutants")
            .args(["--no-times", "--no-shuffle", "--caught", "-Ltrace"])
            .env("INSTA_UPDATE", insta_update)
            .current_dir(tmp_src_dir.path())
            .assert()
            .success();
    }
}

#[test]
fn diff_trees_well_tested() {
    for name in &["diff0", "diff1"] {
        let tmp = copy_of_testdata(name);
        run()
            .args(["mutants", "-d"])
            .arg(tmp.path())
            .assert()
            .success();
    }
}

#[test]
fn list_mutants_changed_in_diff1() {
    let src0 = read_to_string("testdata/diff0/src/lib.rs").unwrap();
    let src1 = read_to_string("testdata/diff1/src/lib.rs").unwrap();
    let diff = TextDiff::from_lines(&src0, &src1)
        .unified_diff()
        .context_radius(2)
        .header("a/src/lib.rs", "b/src/lib.rs")
        .to_string();
    println!("{diff}");

    let mut diff_file = NamedTempFile::new().unwrap();
    diff_file.write_all(diff.as_bytes()).unwrap();

    let tmp = copy_of_testdata("diff1");

    run()
        .args(["mutants", "--no-shuffle", "-d"])
        .arg(tmp.path())
        .arg("--in-diff")
        .arg(diff_file.path())
        .assert()
        .success();

    // Between these trees we just added one function; the existing unchanged
    // function does not need to be tested.
    assert_eq!(
        read_to_string(tmp.path().join("mutants.out/caught.txt")).unwrap(),
        indoc! { "\
            src/lib.rs:6:5: replace two -> String with String::new()
            src/lib.rs:6:5: replace two -> String with \"xyzzy\".into()
        "}
    );

    let mutants_json: serde_json::Value =
        serde_json::from_str(&read_to_string(tmp.path().join("mutants.out/mutants.json")).unwrap())
            .unwrap();
    assert_eq!(
        mutants_json
            .as_array()
            .expect("mutants.json contains an array")
            .len(),
        2
    );
}

#[test]
fn binary_diff_is_not_an_error_and_matches_nothing() {
    // From https://github.com/sourcefrog/cargo-mutants/issues/391
    let mut diff_file = NamedTempFile::new().unwrap();
    diff_file.write_all(b"Binary files a/test-renderers/expected/renderers/fog-None-wgpu.png and b/test-renderers/expected/renderers/fog-None-wgpu.png differ\n").unwrap();
    let tmp = copy_of_testdata("diff1");
    run()
        .args(["mutants", "--no-shuffle", "-d"])
        .arg(tmp.path())
        .arg("--in-diff")
        .arg(diff_file.path())
        .arg("--list")
        .assert()
        .success()
        .stdout("")
        .stderr(predicate::str::contains("INFO Diff file is empty"));
}

#[test]
fn empty_diff_is_not_an_error_and_matches_nothing() {
    let diff_file = NamedTempFile::new().unwrap();
    let tmp = copy_of_testdata("diff1");
    run()
        .args(["mutants", "--no-shuffle", "-d"])
        .arg(tmp.path())
        .arg("--in-diff")
        .arg(diff_file.path())
        .arg("--list")
        .assert()
        .success()
        .stdout("")
        .stderr(predicate::str::contains("INFO Diff file is empty"));
}

// <https://github.com/sourcefrog/cargo-mutants/issues/547>
#[test]
fn diff_containing_non_utf8_is_not_an_error() {
    let mut diff_file = NamedTempFile::new().unwrap();
    diff_file
        .write_all(
            b"--- b   2025-10-05 09:13:10.260014347 -0700
+++ b.8859-1    2025-10-05 09:13:46.056014914 -0700
@@ -1 +1 @@
-\xc3\x9f
+\xdf
",
        )
        .unwrap();
    let tmp = copy_of_testdata("diff1");
    run()
        .args(["mutants", "--no-shuffle", "-d"])
        .arg(tmp.path())
        .arg("--in-diff")
        .arg(diff_file.path())
        .arg("--list")
        .assert()
        .success()
        .stdout("")
        .stderr(" INFO Diff changes no Rust source files\n");
}

/// If the text in the diff doesn't look like the tree then error out.
#[test]
fn mismatched_diff_causes_error() {
    let src0 = read_to_string("testdata/diff0/src/lib.rs").unwrap();
    let src1 = read_to_string("testdata/diff1/src/lib.rs").unwrap();
    let diff = TextDiff::from_lines(&src0, &src1)
        .unified_diff()
        .context_radius(2)
        .header("a/src/lib.rs", "b/src/lib.rs")
        .to_string();
    let diff = diff.replace("fn", "FUNCTION");
    println!("{diff}");

    let mut diff_file = NamedTempFile::new().unwrap();
    diff_file.write_all(diff.as_bytes()).unwrap();

    let tmp = copy_of_testdata("diff1");

    run()
        .args(["mutants", "--no-shuffle", "-d"])
        .arg(tmp.path())
        .arg("--in-diff")
        .arg(diff_file.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "Diff content doesn't match source file: src/lib.rs",
        ));
}

/// If the diff contains multiple deletions (with a new filename of /dev/null),
/// don't fail.
///
/// <https://github.com/sourcefrog/cargo-mutants/issues/219>
#[test]
fn diff_with_multiple_deletions_is_ok() {
    let diff = indoc! {r#"
        diff --git a/src/monitor/collect.rs b/src/monitor/collect.rs
        deleted file mode 100644
        index d842cf9..0000000
        --- a/src/monitor/collect.rs
        +++ /dev/null
        @@ -1,1 +0,0 @@
        -// Some stuff
        diff --git a/src/monitor/another.rs b/src/monitor/another.rs
        deleted file mode 100644
        index d842cf9..0000000
        --- a/src/monitor/collect.rs
        +++ /dev/null
        @@ -1,1 +0,0 @@
        -// More stuff
    "#};
    let mut diff_file = NamedTempFile::new().unwrap();
    diff_file.write_all(diff.as_bytes()).unwrap();

    let tmp = copy_of_testdata("diff1");

    run()
        .args(["mutants", "--no-shuffle", "-d"])
        .arg(tmp.path())
        .arg("--in-diff")
        .arg(diff_file.path())
        .assert()
        .stderr(predicates::str::contains(
            "INFO Diff changes no Rust source files",
        ))
        .success();
}

#[test]
fn in_place_check_leaves_no_changes() -> anyhow::Result<()> {
    fn check_file(tmp: &Path, new_name: &str, old_name: &str) -> anyhow::Result<()> {
        let orig_path = Path::new("testdata/small_well_tested");
        println!("comparing {new_name} and {old_name}");
        assert_eq!(
            read_to_string(tmp.join(new_name))?.replace("\r\n", "\n"),
            read_to_string(orig_path.join(old_name))?.replace("\r\n", "\n"),
            "{old_name} should be the same as {new_name}"
        );
        Ok(())
    }

    let tmp = copy_of_testdata("small_well_tested");
    let tmp_path = tmp.path();
    let output_tmp = TempDir::with_prefix("in_place_check_leaves_no_changes").unwrap();
    let cmd = run()
        .args(["mutants", "--in-place", "--check", "-d"])
        .arg(tmp.path())
        .arg("-o")
        .arg(output_tmp.path())
        .assert()
        .success();
    println!(
        "stdout:\n{}",
        String::from_utf8_lossy(&cmd.get_output().stdout)
    );
    println!(
        "stderr:\n{}",
        String::from_utf8_lossy(&cmd.get_output().stderr)
    );
    check_file(tmp_path, "Cargo.toml", "Cargo_test.toml")?;
    check_file(tmp_path, "src/lib.rs", "src/lib.rs")?;
    Ok(())
}

#[test]
fn error_value_catches_untested_ok_case() {
    // By default this tree should fail because it's configured to
    // generate an error value, and the tests forgot to check that
    // the code under test does return Ok.
    let tmp = copy_of_testdata("error_value");
    run()
        .arg("mutants")
        .args(["-v", "-V", "--no-times", "--no-shuffle"])
        .arg("-d")
        .arg(tmp.path())
        .assert()
        .code(2)
        .stderr("");
}

#[test]
fn no_config_option_disables_config_file_so_error_value_is_not_generated() {
    // In this case, the config file is not loaded. Error values are not
    // generated by default (because we don't know what a good value for
    // this tree would be), so no mutants are caught.
    let tmp_src_dir = copy_of_testdata("error_value");
    run()
        .arg("mutants")
        .args(["-v", "-V", "--no-times", "--no-shuffle", "--no-config"])
        .arg("-d")
        .arg(tmp_src_dir.path())
        .assert()
        .code(0)
        .stderr("")
        .stdout(predicate::function(|stdout: &str| {
            insta::assert_snapshot!(stdout);
            true
        }));
}

#[test]
fn list_mutants_with_error_value_from_command_line_list() {
    // This is not a good error mutant for this tree, which uses
    // anyhow, but it's a good test of the command line option.
    let tmp_src_dir = copy_of_testdata("error_value");
    run()
        .arg("mutants")
        .args([
            "--no-times",
            "--no-shuffle",
            "--no-config",
            "--list",
            "--error=::eyre::eyre!(\"mutant\")",
        ])
        .arg("-d")
        .arg(tmp_src_dir.path())
        .assert()
        .code(0)
        .stderr("")
        .stdout(predicate::function(|stdout: &str| {
            insta::assert_snapshot!(stdout);
            true
        }));
}

#[test]
fn warn_if_error_value_starts_with_err() {
    // Users might misunderstand what should be passed to --error,
    // so give a warning.
    let tmp_src_dir = copy_of_testdata("error_value");
    run()
        .arg("mutants")
        .args([
            "--no-times",
            "--no-shuffle",
            "--no-config",
            "--list",
            "--error=Err(anyhow!(\"mutant\"))",
        ])
        .arg("-d")
        .arg(tmp_src_dir.path())
        .assert()
        .code(0)
        .stderr(predicate::str::contains(
            "error_value option gives the value of the error, and probably should not start with Err(: got Err(anyhow!(\"mutant\"))"
        ));
}

#[test]
fn warn_unresolved_module() {
    let tmp_src_dir = copy_of_testdata("dangling_mod");
    run()
        .arg("mutants")
        .args(["--no-times", "--no-shuffle", "--no-config", "--list"])
        .arg("-d")
        .arg(tmp_src_dir.path())
        .assert()
        .code(0)
        .stderr(predicate::str::contains(
            r#"referent of mod not found definition_site="src/main.rs:3:1" mod_name=nonexistent"#,
        ));
}
#[test]
fn warn_module_outside_of_tree() {
    // manually copy tree, so that external path still resolves correctly for `cargo`
    //
    // [TEMP]/dangling_mod/*
    // [TEMP]/nested_mod/src/paths_in_main/a/foo.rs
    //
    let tree_name = "dangling_mod";
    let tmp_src_dir_parent = TempDir::with_prefix("warn_module_outside_of_tree").unwrap();
    let tmp_src_dir = tmp_src_dir_parent.path().join("dangling_mod");
    cp_r::CopyOptions::new()
        .filter(|path, _stat| {
            Ok(["target", "mutants.out", "mutants.out.old"]
                .iter()
                .all(|p| !path.starts_with(p)))
        })
        .copy_tree(
            std::path::Path::new("testdata").join(tree_name),
            &tmp_src_dir,
        )
        .unwrap();
    rename(
        tmp_src_dir.join("Cargo_test.toml"),
        tmp_src_dir.join("Cargo.toml"),
    )
    .unwrap();

    let external_file_path = tmp_src_dir_parent
        .path()
        .join("nested_mod/src/paths_in_main/a");
    create_dir_all(&external_file_path).unwrap();
    std::fs::copy(
        std::path::Path::new("testdata/nested_mod/src/paths_in_main/a/foo.rs"),
        external_file_path.join("foo.rs"),
    )
    .unwrap();

    run()
        .arg("mutants")
        .args(["--no-times", "--no-shuffle", "--no-config", "--list"])
        .arg("-d")
        .arg(tmp_src_dir)
        .assert()
        .code(0)
        .stderr(predicate::str::contains(
            r#"skipping source outside of tree: "src/../../nested_mod/src/paths_in_main/a/foo.rs""#,
        ));
}

#[test]
fn fail_when_error_value_does_not_parse() {
    let tmp_src_dir = copy_of_testdata("error_value");
    run()
        .arg("mutants")
        .args([
            "--no-times",
            "--no-shuffle",
            "--no-config",
            "--list",
            "--error=shouldn't work",
        ])
        .arg("-d")
        .arg(tmp_src_dir.path())
        .assert()
        .code(1)
        .stderr(predicate::str::contains(indoc! { "
            Error: Failed to parse error value \"shouldn\'t work\"

            Caused by:
                unexpected token
        "}))
        .stdout(predicate::str::is_empty());
}

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

#[test]
fn small_well_tested_tree_check_only() {
    let tmp_src_dir = copy_of_testdata("small_well_tested");
    run()
        .args(["mutants", "--check", "--no-shuffle", "--no-times"])
        .current_dir(tmp_src_dir.path())
        .assert()
        .success()
        .stdout(indoc! {r"
            Found 4 mutants to test
            ok       Unmutated baseline
            ok       src/lib.rs:5:5: replace factorial -> u32 with 0
            ok       src/lib.rs:5:5: replace factorial -> u32 with 1
            ok       src/lib.rs:7:11: replace *= with += in factorial
            ok       src/lib.rs:7:11: replace *= with /= in factorial
            4 mutants tested: 4 succeeded
        "})
        .stderr("");
    let outcomes = outcome_json_counts(&tmp_src_dir);
    assert_eq!(
        outcomes,
        serde_json::json!({
            "success": 4, // They did all build
            "caught": 0, // They weren't actually tested
            "unviable": 0,
            "missed": 0,
            "timeout": 0,
            "total_mutants": 4,
        })
    );
}

#[test]
fn small_well_tested_tree_check_only_shuffled() {
    let tmp_src_dir = copy_of_testdata("small_well_tested");
    run()
        .args(["mutants", "--check", "--no-times", "--shuffle"])
        .current_dir(tmp_src_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("4 mutants tested: 4 succeeded"));
    assert_eq!(
        outcome_json_counts(&tmp_src_dir),
        serde_json::json!({
            "success": 4, // They did all build
            "caught": 0, // They weren't actually tested
            "unviable": 0,
            "missed": 0,
            "timeout": 0,
            "total_mutants": 4,
        })
    );
}

#[test]
fn warning_when_no_mutants_found() {
    let tmp_src_dir = copy_of_testdata("everything_skipped");
    run()
        .args(["mutants", "--check", "--no-times", "--no-shuffle"])
        .current_dir(tmp_src_dir.path())
        .assert()
        .stderr(predicate::str::contains(
            "No mutants found under the active filters",
        ))
        .stdout(predicate::str::contains("Found 0 mutants to test"))
        .success(); // It's arguable, but better if CI doesn't fail in this case.
                    // There is no outcomes.json? Arguably a bug.
}

#[test]
fn check_succeeds_in_tree_that_builds_but_fails_tests() {
    // --check doesn't actually run the tests so won't discover that they fail.
    let tmp_src_dir = copy_of_testdata("already_failing_tests");
    run()
        .args(["mutants", "--check", "--no-times", "--no-shuffle"])
        .current_dir(tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .assert()
        .success()
        .stdout(predicate::function(|stdout: &str| {
            insta::assert_snapshot!(stdout, @r###"
            Found 4 mutants to test
            ok       Unmutated baseline
            ok       src/lib.rs:2:5: replace factorial -> u32 with 0
            ok       src/lib.rs:2:5: replace factorial -> u32 with 1
            ok       src/lib.rs:4:11: replace *= with += in factorial
            ok       src/lib.rs:4:11: replace *= with /= in factorial
            4 mutants tested: 4 succeeded
            "###);
            true
        }));
    assert_eq!(
        outcome_json_counts(&tmp_src_dir),
        serde_json::json!({
            "caught": 0,
            "missed": 0,
            "success": 4,
            "timeout": 0,
            "unviable": 0,
            "total_mutants": 4,
        })
    );
}

#[test]
fn check_tree_with_mutants_skip() {
    let tmp_src_dir = copy_of_testdata("hang_avoided_by_attr");
    run()
        .arg("mutants")
        .args(["--check", "--no-times", "--no-shuffle"])
        .current_dir(tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .assert()
        .success()
        .stdout(indoc! { r"
            Found 6 mutants to test
            ok       Unmutated baseline
            ok       src/lib.rs:15:5: replace controlled_loop with ()
            ok       src/lib.rs:21:28: replace > with == in controlled_loop
            ok       src/lib.rs:21:28: replace > with < in controlled_loop
            ok       src/lib.rs:21:28: replace > with >= in controlled_loop
            ok       src/lib.rs:21:53: replace * with + in controlled_loop
            ok       src/lib.rs:21:53: replace * with / in controlled_loop
            6 mutants tested: 6 succeeded
            "})
        .stderr("");
    assert_eq!(
        outcome_json_counts(&tmp_src_dir),
        serde_json::json!({
            "caught": 0,
            "missed": 0,
            "success": 6,
            "timeout": 0,
            "unviable": 0,
            "total_mutants": 6,
        })
    );
}

#[test]
fn check_tree_where_build_fails() {
    let tmp_src_dir = copy_of_testdata("typecheck_fails");
    run()
        .arg("mutants")
        .args(["--check", "--no-times", "--no-shuffle"])
        .current_dir(tmp_src_dir.path())
        .env_remove("RUST_BACKTRACE")
        .assert()
        .code(4) // clean tests failed
        .stdout(predicate::str::contains("FAILED   Unmutated baseline"));
    assert_eq!(
        outcome_json_counts(&tmp_src_dir),
        serde_json::json!({
            "caught": 0,
            "missed": 0,
            "success": 0,
            "timeout": 0,
            "unviable": 0,
            "total_mutants": 0,
        })
    );
}

#[test]
fn unviable_mutation_of_struct_with_no_default() {
    let tmp_src_dir = copy_of_testdata("struct_with_no_default");
    run()
        .args([
            "mutants",
            "--line-col=false",
            "--check",
            "--no-times",
            "--no-shuffle",
            "-v",
            "-V",
        ])
        .arg("-d")
        .arg(tmp_src_dir.path())
        .assert()
        .success()
        .stdout(
            predicate::str::is_match(
                r"unviable *src/lib.rs:\d+:\d+: replace make_an_s -> S with Default::default\(\)",
            )
            .unwrap(),
        );
    assert_eq!(
        outcome_json_counts(&tmp_src_dir),
        serde_json::json!({
            "success": 0,
            "caught": 0,
            "unviable": 1,
            "missed": 0,
            "timeout": 0,
            "total_mutants": 1,
        })
    );
}

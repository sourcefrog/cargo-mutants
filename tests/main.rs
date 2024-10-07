// Copyright 2021-2024 Martin Pool

//! Tests for cargo-mutants CLI layer.

use std::collections::HashSet;
use std::env;
use std::fs::{self, create_dir, read_dir, read_to_string};
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

use indoc::indoc;
use itertools::Itertools;
use predicate::str::{contains, is_match};
use predicates::prelude::*;
use pretty_assertions::assert_eq;

use tempfile::TempDir;

mod util;
use util::{copy_of_testdata, copy_testdata_to, run, MAIN_BINARY, OUTER_TIMEOUT};

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
fn unviable_mutation_of_struct_with_no_default() {
    let tmp_src_dir = copy_of_testdata("struct_with_no_default");
    run()
        .args([
            "mutants",
            "--line-col=false",
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
    check_text_list_output(
        tmp_src_dir.path(),
        "unviable_mutation_of_struct_with_no_default",
    );
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
        .stderr(predicate::str::contains("WARN").or(predicate::str::contains("ERR")).not())
        .stdout(is_match(
    r"ok *Unmutated baseline in \d+\.\ds"
        ).unwrap())
        .stdout(is_match(
    r"MISSED *src/bin/factorial\.rs:\d+:\d+: replace main with \(\) in \d+\.\ds"
        ).unwrap())
        .stdout(is_match(
    r"caught *src/bin/factorial\.rs:\d+:\d+: replace factorial -> u32 with 0 in \d+\.\ds"
        ).unwrap());
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
    regex::Regex::new(r"cargo.* test --no-run --verbose --manifest-path .* --release")
        .unwrap()
        .captures(&log_content)
        .unwrap();
    regex::Regex::new(r"cargo.* test --verbose --manifest-path .* --release")
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
            insta::assert_snapshot!(stdout);
            true
        }));
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
        .stdout(predicate::function(|stdout: &str| {
            insta::assert_snapshot!(stdout);
            true
        }));
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
                .and(predicate::str::contains("thread 'test_factorial' panicked"))
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
        .stdout(is_match(r"FAILED *Unmutated baseline in \d+\.\ds").unwrap())
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
        .stderr(predicate::str::contains("Auto-set test timeout to 20m 34s"));
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
        .stdout(is_match(r"TIMEOUT *Unmutated baseline in \d+\.\ds").unwrap())
        .stderr(contains(
            "cargo test failed in an unmutated tree, so no mutants were tested",
        ));
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
#[test]
#[cfg(unix)] // Should in principle work on Windows, but does not at the moment.
fn interrupt_caught_and_kills_children() {
    // Test a tree that has enough tests that we'll probably kill it before it completes.

    use std::process::{Command, Stdio};

    use nix::libc::pid_t;
    use nix::sys::signal::{kill, SIGTERM};
    use nix::unistd::Pid;

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
///
/// * `should_stop_const` could change to always return `false`, in which
///   case the loop in the block for the const `VAL` will never stop, but
///   the build should eventually be killed by a timeout.
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
        .expect("read timeout.txt");
    let caught_txt = read_to_string(tmp_src_dir.path().join("mutants.out/caught.txt"))
        .expect("read timeout.txt");
    let timeout_txt = read_to_string(tmp_src_dir.path().join("mutants.out/timeout.txt"))
        .expect("read timeout.txt");
    assert!(
        timeout_txt.contains("replace should_stop -> bool with false"),
        "expected text not found in:\n{timeout_txt}"
    );
    assert!(
        unviable_txt.contains("replace should_stop_const -> bool with false"),
        "expected text not found in:\n{unviable_txt}"
    );
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

    let phases_for_const_fn = outcomes_json["outcomes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|outcome| {
            outcome["scenario"]["Mutant"]["function"]["function_name"] == "should_stop_const"
        })
        .flat_map(|outcome| outcome["phase_results"].as_array())
        .next()
        .expect("Failed to find phase_results for 'should_stop_const' fn");

    assert_eq!(phases_for_const_fn.len(), 1);
    assert_eq!(phases_for_const_fn[0]["phase"], "Build");
}

#[test]
fn hang_avoided_by_build_timeout_with_cap_lints() {
    let tmp_src_dir = copy_of_testdata("hang_when_mutated");
    let out = run()
        .arg("mutants")
        .args([
            "--build-timeout-multiplier=4",
            "--regex=const",
            "--cap-lints=true",
        ])
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
    assert!(
        timeout_txt.contains("replace should_stop_const -> bool with false"),
        "expected text not found in:\n{timeout_txt}"
    );
}

#[test]
fn constfn_mutation_passes_check() {
    let tmp_src_dir = copy_of_testdata("hang_when_mutated");
    let cmd = run()
        .arg("mutants")
        .args(["--check", "--build-timeout=4"])
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
    cp_r::CopyOptions::new()
        .copy_tree("mutants_attrs", tmp_path.join("mutants_attrs"))
        .unwrap();
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

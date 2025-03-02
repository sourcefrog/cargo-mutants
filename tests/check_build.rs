// Copyright 2024 Martin Pool

//! Tests for `--check`

use indoc::indoc;
use predicates::prelude::*;
use pretty_assertions::assert_eq;

mod util;
use util::{copy_of_testdata, outcome_json_counts, run};

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

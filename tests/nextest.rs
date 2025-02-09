// Copyright 2024-2025 Martin Pool

//! Integration tests for cargo mutants calling nextest.

use std::fs::{create_dir, write};

use predicates::prelude::*;
use tempfile::TempDir;

mod util;
use util::{copy_of_testdata, run};

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

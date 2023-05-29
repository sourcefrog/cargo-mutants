// Copyright 2022 Martin Pool.

//! Test handling of `mutants.toml` configuration.

use std::fs::{create_dir, write};

use predicates::prelude::*;
use tempfile::TempDir;

use super::{copy_of_testdata, run};

fn write_config_file(tempdir: &TempDir, config: &str) {
    let path = tempdir.path();
    // This will error if it exists, which today it never will,
    // but perhaps later we should ignore that.
    create_dir(path.join(".cargo")).unwrap();
    write(path.join(".cargo/mutants.toml"), config.as_bytes()).unwrap();
}

#[test]
fn invalid_toml_rejected() {
    let testdata = copy_of_testdata("well_tested");
    write_config_file(
        &testdata,
        r#"what even is this?
        "#,
    );
    run()
        .args(["mutants", "--list-files", "-d"])
        .arg(testdata.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("Error: parse toml from "));
}

#[test]
fn invalid_field_rejected() {
    let testdata = copy_of_testdata("well_tested");
    write_config_file(
        &testdata,
        r#"wobble = false
        "#,
    );
    run()
        .args(["mutants", "--list-files", "-d"])
        .arg(testdata.path())
        .assert()
        .failure()
        .stderr(
            predicates::str::contains("Error: parse toml from ")
                .and(predicates::str::contains("unknown field `wobble`")),
        );
}

#[test]
fn list_with_config_file_exclusion() {
    let testdata = copy_of_testdata("well_tested");
    write_config_file(
        &testdata,
        r#"exclude_globs = ["src/*_mod.rs"]
        "#,
    );
    run()
        .args(["mutants", "--list-files", "-d"])
        .arg(testdata.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("_mod.rs").not());
    run()
        .args(["mutants", "--list", "-d"])
        .arg(testdata.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("_mod.rs").not());
}

#[test]
fn list_with_config_file_inclusion() {
    let testdata = copy_of_testdata("well_tested");
    write_config_file(
        &testdata,
        r#"examine_globs = ["src/*_mod.rs"]
        "#,
    );
    run()
        .args(["mutants", "--list-files", "-d"])
        .arg(testdata.path())
        .assert()
        .success()
        .stdout(predicates::str::diff(
            "src/inside_mod.rs
src/item_mod.rs\n",
        ));
    run()
        .args(["mutants", "--list", "-d"])
        .arg(testdata.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("simple_fns.rs").not());
}

#[test]
fn list_with_config_file_regexps() {
    let testdata = copy_of_testdata("well_tested");
    write_config_file(
        &testdata,
        r#"
        # comments are ok
        examine_re = ["divisible"]
        exclude_re = ["-> bool with true"]
        "#,
    );
    run()
        .args(["mutants", "--list", "-d"])
        .arg(testdata.path())
        .assert()
        .success()
        .stdout(predicates::str::diff(
            "src/simple_fns.rs:17: replace divisible_by_three -> bool with false\n",
        ));
}

#[test]
fn tree_fails_without_needed_feature() {
    // The point of this tree is to check that Cargo features can be turned on,
    // but let's make sure it does fail as intended if they're not.
    let testdata = copy_of_testdata("fails_without_feature");
    run()
        .args(["mutants", "-d"])
        .arg(testdata.path())
        .assert()
        .failure()
        .stdout(predicates::str::contains(
            "test failed in an unmutated tree",
        ));
}

#[test]
fn additional_cargo_args() {
    // The point of this tree is to check that Cargo features can be turned on,
    // but let's make sure it does fail as intended if they're not.
    let testdata = copy_of_testdata("fails_without_feature");
    write_config_file(
        &testdata,
        r#"
        additional_cargo_args = ["--features", "needed"]
        "#,
    );
    run()
        .args(["mutants", "-d"])
        .arg(testdata.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("2 caught"));
}

#[test]
fn additional_cargo_test_args() {
    // The point of this tree is to check that Cargo features can be turned on,
    // but let's make sure it does fail as intended if they're not.
    let testdata = copy_of_testdata("fails_without_feature");
    write_config_file(
        &testdata,
        r#"
        additional_cargo_test_args = ["--all-features", ]
        "#,
    );
    run()
        .args(["mutants", "-d"])
        .arg(testdata.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("2 caught"));
}

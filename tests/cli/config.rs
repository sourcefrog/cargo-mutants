// Copyright 2022 Martin Pool.

//! Test handling of `mutants.toml` configuration.

use std::fs::{create_dir, write};

use indoc::indoc;
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
        .stdout(predicates::str::diff(indoc! { "\
            src/inside_mod.rs
            src/item_mod.rs
        " }));
    run()
        .args(["mutants", "--list", "-d"])
        .arg(testdata.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("simple_fns.rs").not());
}

#[test]
fn file_argument_overrides_config_examine_globs_key() {
    let testdata = copy_of_testdata("well_tested");
    // This config key has no effect because the command line argument
    // takes precedence.
    write_config_file(
        &testdata,
        r#"examine_globs = ["src/*_mod.rs"]
        "#,
    );
    run()
        .args(["mutants", "--list-files", "-d"])
        .arg(testdata.path())
        .args(["--file", "src/simple_fns.rs"])
        .assert()
        .success()
        .stdout(predicates::str::diff(indoc! { "\
            src/simple_fns.rs
        " }));
}

#[test]
fn exclude_file_argument_overrides_config() {
    let testdata = copy_of_testdata("well_tested");
    // This config key has no effect because the command line argument
    // takes precedence.
    write_config_file(
        &testdata,
        indoc! { r#"
            examine_globs = ["src/*_mod.rs"]
            exclude_globs = ["src/inside_mod.rs"]
        "#},
    );
    run()
        .args(["mutants", "--list-files", "-d"])
        .arg(testdata.path())
        .args(["--file", "src/*.rs"])
        .args(["--exclude", "src/*_mod.rs"])
        .args(["--exclude", "src/s*.rs"])
        .args(["--exclude", "src/n*.rs"])
        .assert()
        .success()
        .stdout(predicates::str::diff(indoc! { "\
            src/lib.rs
            src/arc.rs
            src/empty_fns.rs
            src/methods.rs
            src/result.rs
        " }));
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
        .stdout(predicates::str::diff(indoc! {"\
                src/simple_fns.rs:17: replace divisible_by_three -> bool with false
            "}));
    // src/simple_fns.rs:18: replace == with != in divisible_by_three -> bool
}

#[test]
fn exclude_re_overrides_config() {
    let testdata = copy_of_testdata("well_tested");
    write_config_file(
        &testdata,
        r#"
        exclude_re = [".*"]     # would exclude everything
        "#,
    );
    run()
        .args(["mutants", "--list", "-d"])
        .arg(testdata.path())
        .assert()
        .success()
        .stdout(predicates::str::is_empty());
    // Also tests that the alias --exclude-regex is accepted
    run()
        .args(["mutants", "--list", "-d"])
        .arg(testdata.path())
        .args(["--exclude-regex", " -> "])
        .args(["-f", "src/simple_fns.rs"])
        .assert()
        .success()
        .stdout(indoc! {"
            src/simple_fns.rs:7: replace returns_unit with ()
        "});
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
        .stderr(predicates::str::contains(
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

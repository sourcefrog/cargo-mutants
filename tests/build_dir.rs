// Copyright 2023 Martin Pool

use std::fs::write;

mod util;
use util::{copy_of_testdata, run};

#[test]
fn gitignore_respected_in_copy_by_default() {
    // Make a tree with a (dumb) gitignore that excludes the source file; when you copy it
    // to a build directory, the source file should not be there and so the check will fail.
    let tmp = copy_of_testdata("factorial");
    write(tmp.path().join(".gitignore"), b"src\n").unwrap();
    run()
        .args(["mutants", "--check", "-d"])
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

/// A tree containing a symlink that must exist for the tests to pass works properly.
#[test]
#[cfg(unix)]
fn tree_with_symlink() {
    let tmp = copy_of_testdata("symlink");
    assert!(tmp.path().join("testdata").join("symlink").is_symlink());
    run()
        .args(["mutants", "-d"])
        .arg(tmp.path())
        .assert()
        .success();
}

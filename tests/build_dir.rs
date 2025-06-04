// Copyright 2023-2024 Martin Pool

use std::fs::{create_dir, write};

mod util;
use util::{copy_of_testdata, run};

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

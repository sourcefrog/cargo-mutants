// Copyright 2021 Martin Pool

//! Tests for CLI layer.

use std::path::PathBuf;

// use assert_cmd::prelude::*;
use assert_cmd::Command;

use lazy_static::lazy_static;

#[allow(unused)]
use pretty_assertions::assert_eq;

lazy_static! {
    static ref MAIN_BINARY: PathBuf = assert_cmd::cargo::cargo_bin("enucleate");
}

#[test]
fn list_files_in_factorial() {
    Command::new(MAIN_BINARY.as_os_str())
        .arg("list-files")
        .arg("-d")
        .arg("testdata/tree/factorial")
        .assert()
        .success()
        .stdout("src/bin/main.rs\n")
        .stderr("");
}

#[test]
fn list_mutants_in_factorial() {
    let output = std::process::Command::new(MAIN_BINARY.as_os_str())
        .arg("list-mutants")
        .current_dir("testdata/tree/factorial")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    insta::assert_snapshot!(String::from_utf8_lossy(&output.stdout));
}

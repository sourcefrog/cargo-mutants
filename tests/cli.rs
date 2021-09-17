// Copyright 2021 Martin Pool

//! Tests for CLI layer.

use std::path::PathBuf;
use std::process::Command;

// use assert_cmd::prelude::*;
// use assert_cmd::Command;

use lazy_static::lazy_static;

#[allow(unused)]
use pretty_assertions::assert_eq;

lazy_static! {
    static ref MAIN_BINARY: PathBuf = assert_cmd::cargo::cargo_bin("enucleate");
}

trait CommandInstaExt {
    fn assert_insta(&mut self);
}

impl CommandInstaExt for std::process::Command {
    fn assert_insta(&mut self) {
        let output = self.output().expect("command completes");
        assert!(output.status.success());
        insta::assert_snapshot!(String::from_utf8_lossy(&output.stdout));
        assert_eq!(&String::from_utf8_lossy(&output.stderr), "");
    }
}

#[test]
fn list_files_in_factorial() {
    Command::new(MAIN_BINARY.as_os_str())
        .arg("list-files")
        .arg("-d")
        .arg("testdata/tree/factorial")
        .assert_insta();
}

#[test]
fn list_mutants_in_factorial() {
    Command::new(MAIN_BINARY.as_os_str())
        .arg("list-mutants")
        .current_dir("testdata/tree/factorial")
        .assert_insta();
}

#[test]
fn list_mutants_with_dir_option() {
    Command::new(MAIN_BINARY.as_os_str())
        .arg("list-mutants")
        .arg("--dir")
        .arg("testdata/tree/factorial")
        .assert_insta();
}

#[test]
fn list_mutants_with_diffs_in_factorial() {
    Command::new(MAIN_BINARY.as_os_str())
        .arg("list-mutants")
        .arg("--diff")
        .current_dir("testdata/tree/factorial")
        .assert_insta();
}

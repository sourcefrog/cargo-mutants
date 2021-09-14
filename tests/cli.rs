// Copyright 2021 Martin Pool

//! Tests for CLI layer.

use std::path::PathBuf;

// use assert_cmd::prelude::*;
use assert_cmd::Command;

use lazy_static::lazy_static;

#[allow(unused)]
use pretty_assertions::*;

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

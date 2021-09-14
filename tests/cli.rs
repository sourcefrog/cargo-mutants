// Copyright 2021 Martin Pool

//! Tests for CLI layer.

// use assert_cmd::prelude::*;
use assert_cmd::Command;
#[allow(unused)]
use pretty_assertions::*;

const BIN_NAME: &str = "enucleate";

#[test]
fn list_files_in_factorial() {
    Command::cargo_bin(BIN_NAME)
        .unwrap()
        .arg("list-files")
        .arg("-d")
        .arg("testdata/tree/factorial")
        .assert()
        .success()
        .stdout("src/bin/main.rs\n")
        .stderr("");
}

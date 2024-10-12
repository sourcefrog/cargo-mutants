// Copyright 2024 Martin Pool

#![allow(dead_code)] // rustc doesn't understand they're used by multiple crates

//! Reusable utilities for cargo-mutants tests.
//!
//! This is available both to integration tests (by `mod util`) and unit tests inside
//! cargo-mutants (as `use crate::test_util`).

use std::borrow::Borrow;
use std::env;
use std::fs::{read_dir, read_to_string, rename};
use std::path::{Path, PathBuf};
use std::time::Duration;

use itertools::Itertools;
use lazy_static::lazy_static;
use tempfile::TempDir;

/// A timeout for a `cargo mutants` invocation from the test suite. Needs to be
/// long enough that even commands that do a lot of work can pass even on slow
/// CI VMs and even on Windows, but short enough that the test does not hang
/// forever.
pub const OUTER_TIMEOUT: Duration = Duration::from_secs(60);

lazy_static! {
    pub static ref MAIN_BINARY: PathBuf = assert_cmd::cargo::cargo_bin("cargo-mutants");
}

pub fn run() -> assert_cmd::Command {
    let mut cmd = assert_cmd::Command::new(MAIN_BINARY.as_os_str());
    // Strip any options configured in the environment running these tests,
    // so that they don't cause unexpected behavior in the code under test.
    //
    // For example, without this,
    // `env CARGO_MUTANTS_JOBS=4 cargo mutants`
    //
    // would end up with tests running 4 jobs by default, which would cause
    // the tests to fail.
    //
    // Even more generally than that example, we want the tests to be as hermetic
    // as reasonably possible.
    env::vars()
        .map(|(k, _v)| k)
        .filter(|k| {
            k.starts_with("CARGO_MUTANTS_")
                || k == "CLICOLOR_FORCE"
                || k == "NOCOLOR"
                || k == "CARGO_TERM_COLOR"
        })
        .for_each(|k| {
            cmd.env_remove(k);
        });
    cmd
}

pub trait CommandInstaExt {
    fn assert_insta(&mut self, snapshot_name: &str);
}

impl CommandInstaExt for assert_cmd::Command {
    fn assert_insta(&mut self, snapshot_name: &str) {
        let output = self.output().expect("command completes");
        assert!(output.status.success());
        insta::assert_snapshot!(snapshot_name, String::from_utf8_lossy(&output.stdout));
        assert_eq!(&String::from_utf8_lossy(&output.stderr), "");
    }
}

// Copy the source for one testdata tree.
pub fn copy_of_testdata(tree_name: &str) -> TempDir {
    assert!(
        !tree_name.contains("/"),
        "testdata tree name {tree_name:?} should be just the directory name"
    );
    let tmp = TempDir::with_prefix(format!("cargo-mutants-testdata-{tree_name}-")).unwrap();
    copy_testdata_to(tree_name, tmp.path());
    tmp
}

pub fn copy_testdata_to<P: AsRef<Path>>(tree_name: &str, dest: P) {
    let dest = dest.as_ref();
    let mut cargo_toml_files = Vec::new();
    cp_r::CopyOptions::new()
        .filter(|path, _stat| {
            Ok(["target", "mutants.out", "mutants.out.old"]
                .iter()
                .all(|p| !path.starts_with(p)))
        })
        .after_entry_copied(|path, file_type, _stats| {
            if file_type.is_file() && path.ends_with("Cargo_test.toml") {
                cargo_toml_files.push(dest.join(path))
            }
            Ok(())
        })
        .copy_tree(Path::new("testdata").join(tree_name), dest)
        .unwrap();
    for path in &cargo_toml_files {
        if let Err(err) = rename(path, path.parent().unwrap().join("Cargo.toml")) {
            panic!("failed to rename {path:?}: {err:?}")
        }
    }
}

/// Assert that some bytes, when parsed as json, equal a json value.
pub fn assert_bytes_eq_json<J: Borrow<serde_json::Value>>(actual: &[u8], expected: J) {
    // The Borrow is so that you can pass either a value or a reference, for easier
    // calling.
    let actual_json = std::str::from_utf8(actual)
        .expect("bytes are UTF-8")
        .parse::<serde_json::Value>()
        .expect("bytes can be parsed as JSON");
    assert_eq!(&actual_json, expected.borrow());
}

/// Return paths to all testdata trees, in order, excluding leftover git
/// detritus with no Cargo.toml.
pub fn all_testdata_tree_names() -> Vec<String> {
    read_dir("testdata")
        .expect("list testdata")
        .map(|r| r.expect("read testdata dir entry"))
        .filter(|dir_entry| dir_entry.file_type().unwrap().is_dir())
        .filter(|dir_entry| dir_entry.file_name() != "parse_fails")
        .filter(|dir_entry| {
            let dir_path = dir_entry.path();
            dir_path.join("Cargo.toml").exists() || dir_path.join("Cargo_test.toml").exists()
        })
        .map(|dir_entry| {
            dir_entry
                .file_name()
                .into_string()
                .expect("dir name is UTF-8")
        })
        .sorted()
        .collect()
}

pub fn outcome_json(tmp_src_dir: &TempDir) -> serde_json::Value {
    read_to_string(tmp_src_dir.path().join("mutants.out/outcomes.json"))
        .expect("read outcomes.json")
        .parse()
        .expect("parse outcomes.json")
}

pub fn outcome_json_counts(tmp_src_dir: &TempDir) -> serde_json::Value {
    let mut outcomes = outcome_json(tmp_src_dir);
    // We don't want to compare the detailed outcomes
    outcomes.as_object_mut().unwrap().remove("outcomes");
    outcomes
}

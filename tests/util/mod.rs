// Copyright 2024 Martin Pool

#![allow(dead_code)] // rustc doesn't understand they're used by multiple crates?

//! Reusable utilities for cargo-mutants tests.

use std::borrow::Borrow;
use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;

use lazy_static::lazy_static;
use tempfile::{tempdir, TempDir};

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

// Copy the source because output is written into mutants.out.
pub fn copy_of_testdata(tree_name: &str) -> TempDir {
    let tmp_src_dir = tempdir().unwrap();
    cp_r::CopyOptions::new()
        .filter(|path, _stat| {
            Ok(["target", "mutants.out", "mutants.out.old"]
                .iter()
                .all(|p| !path.starts_with(p)))
        })
        .copy_tree(Path::new("testdata").join(tree_name), &tmp_src_dir)
        .unwrap();
    tmp_src_dir
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

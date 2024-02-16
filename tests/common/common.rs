// Copyright 2021-2024 Martin Pool

//! Common code across all integration tests.

#![allow(dead_code)] // rustc can't tell these are used

use std::env;
use std::path::{Path, PathBuf};

use lazy_static::lazy_static;
use tempfile::{tempdir, TempDir};

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

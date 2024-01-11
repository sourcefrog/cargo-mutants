// Copyright 2024 Martin Pool

//! Integration tests for cargo mutants calling nextest.

use super::{copy_of_testdata, run};

#[test]
fn test_with_nextest_on_small_tree() {
    let tmp_src_dir = copy_of_testdata("small_well_tested");
    let assert = run()
        .args(["mutants", "--test-tool", "nextest", "-vV", "--no-shuffle"])
        .arg("-d")
        .arg(tmp_src_dir.path())
        .assert()
        .success();
    println!(
        "stdout:\n{}",
        String::from_utf8_lossy(&assert.get_output().stdout)
    );
}

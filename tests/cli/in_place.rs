// Copyright 2024 Martin Pool

//! Test `--in-place` behavior.

use std::path::Path;

use anyhow::Result;
use tempfile::TempDir;

use crate::copy_of_testdata;

use super::run;

#[test]
fn in_place_check_leaves_no_changes() -> Result<()> {
    let tmp = copy_of_testdata("small_well_tested");
    let output_tmp = TempDir::new().unwrap();
    let cmd = run()
        .args(["mutants", "--in-place", "--check", "-d"])
        .arg(tmp.path())
        .arg("-o")
        .arg(output_tmp.path())
        .assert()
        .success();
    println!(
        "stdout:\n{}",
        String::from_utf8_lossy(&cmd.get_output().stdout)
    );
    println!(
        "stderr:\n{}",
        String::from_utf8_lossy(&cmd.get_output().stderr)
    );
    let orig_path = Path::new("testdata/small_well_tested");
    for filename in ["Cargo.toml", "src/lib.rs"] {
        println!("comparing {filename}");
        assert_eq!(
            std::fs::read_to_string(tmp.path().join(filename))?.replace("\r\n", "\n"),
            std::fs::read_to_string(orig_path.join(filename))?.replace("\r\n", "\n"),
            "{filename} should be unchanged"
        );
    }
    Ok(())
}

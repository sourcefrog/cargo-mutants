// Copyright 2024 Martin Pool

//! Test `--in-place` behavior.

use std::{fs::read_to_string, path::Path};

use anyhow::Result;
use tempfile::TempDir;

mod util;
use util::{copy_of_testdata, run};

#[test]
fn in_place_check_leaves_no_changes() -> Result<()> {
    let tmp = copy_of_testdata("small_well_tested");
    let tmp_path = tmp.path();
    let output_tmp = TempDir::with_prefix("in_place_check_leaves_no_changes").unwrap();
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
    fn check_file(tmp: &Path, new_name: &str, old_name: &str) -> Result<()> {
        let orig_path = Path::new("testdata/small_well_tested");
        println!("comparing {new_name} and {old_name}");
        assert_eq!(
            read_to_string(tmp.join(new_name))?.replace("\r\n", "\n"),
            read_to_string(orig_path.join(old_name))?.replace("\r\n", "\n"),
            "{old_name} should be the same as {new_name}"
        );
        Ok(())
    }
    check_file(tmp_path, "Cargo.toml", "Cargo_test.toml")?;
    check_file(tmp_path, "src/lib.rs", "src/lib.rs")?;
    Ok(())
}

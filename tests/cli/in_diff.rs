// Copyright 2023 Martin Pool

use std::fs::read_to_string;
use std::io::Write;

use indoc::indoc;
use similar::TextDiff;
use tempfile::NamedTempFile;

use super::{copy_of_testdata, run};

#[test]
fn diff_trees_well_tested() {
    for name in &["diff0", "diff1"] {
        let tmp = copy_of_testdata(name);
        run()
            .args(["mutants", "-d"])
            .arg(tmp.path())
            .assert()
            .success();
    }
}

#[test]
fn list_mutants_changed_in_diff1() {
    let src0 = read_to_string("testdata/tree/diff0/src/lib.rs").unwrap();
    let src1 = read_to_string("testdata/tree/diff1/src/lib.rs").unwrap();
    let diff = TextDiff::from_lines(&src0, &src1)
        .unified_diff()
        .context_radius(2)
        .header("a/src/lib.rs", "b/src/lib.rs")
        .to_string();
    println!("{diff}");

    let mut diff_file = NamedTempFile::new().unwrap();
    diff_file.write_all(diff.as_bytes()).unwrap();

    let tmp = copy_of_testdata("diff1");

    run()
        .args(["mutants", "--no-shuffle", "-d"])
        .arg(tmp.path())
        .arg("--in-diff")
        .arg(diff_file.path())
        .assert()
        .success();

    // Between these trees we just added one function; the existing unchanged
    // function does not need to be tested.
    assert_eq!(
        read_to_string(tmp.path().join("mutants.out/caught.txt")).unwrap(),
        indoc! { "\
            src/lib.rs:5: replace two -> String with String::new()
            src/lib.rs:5: replace two -> String with \"xyzzy\".into()
        "}
    );

    let mutants_json: serde_json::Value =
        serde_json::from_str(&read_to_string(tmp.path().join("mutants.out/mutants.json")).unwrap())
            .unwrap();
    assert_eq!(
        mutants_json
            .as_array()
            .expect("mutants.json contains an array")
            .len(),
        2
    );
}

/// If the text in the diff doesn't look like the tree then error out.
#[test]
fn mismatched_diff_causes_error() {
    let src0 = read_to_string("testdata/tree/diff0/src/lib.rs").unwrap();
    let src1 = read_to_string("testdata/tree/diff1/src/lib.rs").unwrap();
    let diff = TextDiff::from_lines(&src0, &src1)
        .unified_diff()
        .context_radius(2)
        .header("a/src/lib.rs", "b/src/lib.rs")
        .to_string();
    let diff = diff.replace("fn", "FUNCTION");
    println!("{diff}");

    let mut diff_file = NamedTempFile::new().unwrap();
    diff_file.write_all(diff.as_bytes()).unwrap();

    let tmp = copy_of_testdata("diff1");

    run()
        .args(["mutants", "--no-shuffle", "-d"])
        .arg(tmp.path())
        .arg("--in-diff")
        .arg(diff_file.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "Diff content doesn't match source file: src/lib.rs",
        ));
}

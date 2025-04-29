// Copyright 2023 Martin Pool

use std::fs::read_to_string;
use std::io::Write;

use indoc::indoc;
use predicates::prelude::predicate;
use similar::TextDiff;
use tempfile::NamedTempFile;

mod util;
use util::{copy_of_testdata, run};

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
    let src0 = read_to_string("testdata/diff0/src/lib.rs").unwrap();
    let src1 = read_to_string("testdata/diff1/src/lib.rs").unwrap();
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
            src/lib.rs:6:5: replace two -> String with String::new()
            src/lib.rs:6:5: replace two -> String with \"xyzzy\".into()
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

#[test]
fn binary_diff_is_not_an_error_and_matches_nothing() {
    // From https://github.com/sourcefrog/cargo-mutants/issues/391
    let mut diff_file = NamedTempFile::new().unwrap();
    diff_file.write_all(b"Binary files a/test-renderers/expected/renderers/fog-None-wgpu.png and b/test-renderers/expected/renderers/fog-None-wgpu.png differ\n").unwrap();
    let tmp = copy_of_testdata("diff1");
    run()
        .args(["mutants", "--no-shuffle", "-d"])
        .arg(tmp.path())
        .arg("--in-diff")
        .arg(diff_file.path())
        .arg("--list")
        .assert()
        .success()
        .stdout("")
        .stderr("");
}

#[test]
fn empty_diff_is_not_an_error_and_matches_nothing() {
    let diff_file = NamedTempFile::new().unwrap();
    let tmp = copy_of_testdata("diff1");
    run()
        .args(["mutants", "--no-shuffle", "-d"])
        .arg(tmp.path())
        .arg("--in-diff")
        .arg(diff_file.path())
        .arg("--list")
        .assert()
        .success()
        .stdout("")
        .stderr(predicate::str::contains("diff file is empty"));
}

/// If the text in the diff doesn't look like the tree then error out.
#[test]
fn mismatched_diff_causes_error() {
    let src0 = read_to_string("testdata/diff0/src/lib.rs").unwrap();
    let src1 = read_to_string("testdata/diff1/src/lib.rs").unwrap();
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

/// If the diff contains multiple deletions (with a new filename of /dev/null),
/// don't fail.
///
/// <https://github.com/sourcefrog/cargo-mutants/issues/219>
#[test]
fn diff_with_multiple_deletions_is_ok() {
    let diff = indoc! {r#"
        diff --git a/src/monitor/collect.rs b/src/monitor/collect.rs
        deleted file mode 100644
        index d842cf9..0000000
        --- a/src/monitor/collect.rs
        +++ /dev/null
        @@ -1,1 +0,0 @@
        -// Some stuff
        diff --git a/src/monitor/another.rs b/src/monitor/another.rs
        deleted file mode 100644
        index d842cf9..0000000
        --- a/src/monitor/collect.rs
        +++ /dev/null
        @@ -1,1 +0,0 @@
        -// More stuff
    "#};
    let mut diff_file = NamedTempFile::new().unwrap();
    diff_file.write_all(diff.as_bytes()).unwrap();

    let tmp = copy_of_testdata("diff1");

    run()
        .args(["mutants", "--no-shuffle", "-d"])
        .arg(tmp.path())
        .arg("--in-diff")
        .arg(diff_file.path())
        .assert()
        .stderr(predicates::str::contains(
            "No mutants found under the active filters",
        ))
        .success();
}

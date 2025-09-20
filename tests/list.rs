// Copyright 2021-2024 Martin Pool

//! Tests for cargo-mutants `--list` and filtering of the list.

use predicates::prelude::*;
use pretty_assertions::assert_eq;

mod util;
use util::{copy_of_testdata, run, CommandInstaExt};

#[test]
fn list_diff_json_contains_diffs() {
    let tmp = copy_of_testdata("factorial");
    let cmd = run()
        .args(["mutants", "--list", "--json", "--diff", "-d"])
        .arg(tmp.path())
        .assert()
        .success(); // needed for lifetime
    let out = cmd.get_output();
    assert_eq!(String::from_utf8_lossy(&out.stderr), "");
    println!("{}", String::from_utf8_lossy(&out.stdout));
    let out_json = serde_json::from_slice::<serde_json::Value>(&out.stdout).unwrap();
    let mutants_json = out_json.as_array().expect("json output is array");
    assert_eq!(mutants_json.len(), 5);
    assert!(mutants_json.iter().all(|e| e.as_object().unwrap()["diff"]
        .as_str()
        .unwrap()
        .contains("--- src/bin/factorial.rs")));
}

#[test]
fn list_mutants_in_factorial() {
    let tmp = copy_of_testdata("factorial");
    run()
        .arg("mutants")
        .arg("--list")
        .current_dir(&tmp)
        .assert_insta("list_mutants_in_factorial");
}

#[test]
fn list_mutants_in_factorial_json() {
    let tmp = copy_of_testdata("factorial");
    run()
        .arg("mutants")
        .arg("--list")
        .arg("--json")
        .current_dir(tmp.path())
        .assert_insta("list_mutants_in_factorial_json");
}

#[test]
fn list_mutants_in_cfg_attr_mutants_skip() {
    let tmp_src_dir = copy_of_testdata("cfg_attr_mutants_skip");
    run()
        .arg("mutants")
        .arg("--list")
        .current_dir(tmp_src_dir.path())
        .assert_insta("list_mutants_in_cfg_attr_mutants_skip");
}

#[test]
fn list_mutants_in_cfg_attr_mutants_skip_json() {
    let tmp_src_dir = copy_of_testdata("cfg_attr_mutants_skip");
    run()
        .arg("mutants")
        .arg("--list")
        .arg("--json")
        .current_dir(tmp_src_dir.path())
        .assert_insta("list_mutants_in_cfg_attr_mutants_skip_json");
}

#[test]
fn list_mutants_in_cfg_attr_test_skip() {
    let tmp_src_dir = copy_of_testdata("cfg_attr_test_skip");
    run()
        .arg("mutants")
        .arg("--list")
        .current_dir(tmp_src_dir.path())
        .assert_insta("list_mutants_in_cfg_attr_test_skip");
}

#[test]
fn list_mutants_in_cfg_attr_test_skip_json() {
    let tmp_src_dir = copy_of_testdata("cfg_attr_test_skip");
    run()
        .arg("mutants")
        .arg("--list")
        .arg("--json")
        .current_dir(tmp_src_dir.path())
        .assert_insta("list_mutants_in_cfg_attr_test_skip_json");
}

#[test]
fn list_mutants_with_dir_option() {
    let temp = copy_of_testdata("factorial");
    run()
        .arg("mutants")
        .arg("--list")
        .arg("--dir")
        .arg(temp.path())
        .assert_insta("list_mutants_with_dir_option");
}

#[test]
fn list_mutants_with_diffs_in_factorial() {
    let tmp = copy_of_testdata("factorial");
    run()
        .arg("mutants")
        .arg("--list")
        .arg("--diff")
        .current_dir(&tmp)
        .assert_insta("list_mutants_with_diffs_in_factorial");
}

#[test]
fn list_mutants_well_tested() {
    let tmp = copy_of_testdata("well_tested");
    run()
        .arg("mutants")
        .arg("--list")
        .current_dir(tmp.path())
        .assert_insta("list_mutants_well_tested");
}

#[test]
fn list_mutants_well_tested_examine_name_filter() {
    let tmp = copy_of_testdata("well_tested");
    run()
        .arg("mutants")
        .args(["--list", "--file", "nested_function.rs"])
        .current_dir(tmp.path())
        .assert_insta("list_mutants_well_tested_examine_name_filter");
}

#[test]
fn list_mutants_well_tested_exclude_name_filter() {
    let tmp = copy_of_testdata("well_tested");
    run()
        .arg("mutants")
        .args(["--list", "--exclude", "simple_fns.rs"])
        .current_dir(tmp.path())
        .assert_insta("list_mutants_well_tested_exclude_name_filter");
}

#[test]
fn list_mutants_well_tested_exclude_folder_filter() {
    let tmp = copy_of_testdata("with_child_directories");
    run()
        .arg("mutants")
        .args(["--list", "--exclude", "module"])
        .current_dir(tmp.path())
        .assert_insta("list_mutants_well_tested_exclude_folder_filter");
}

#[test]
fn list_mutants_well_tested_examine_and_exclude_name_filter_combined() {
    let tmp = copy_of_testdata("with_child_directories");
    run()
        .arg("mutants")
        .args([
            "--list",
            "--file",
            "src/module/utils/**/*.rs",
            "--exclude",
            "nested_function.rs",
        ])
        .current_dir(tmp.path())
        .assert_insta("list_mutants_well_tested_examine_and_exclude_name_filter_combined");
}

#[test]
fn list_mutants_regex_filters() {
    let tmp = copy_of_testdata("well_tested");
    run()
        .arg("mutants")
        .args(["--list", "--re", "divisible"])
        .arg("-d")
        .arg(tmp.path())
        .assert_insta("list_mutants_regex_filters");
}

#[test]
fn list_mutants_regex_anchored_matches_full_line() {
    let tmp = copy_of_testdata("well_tested");
    run()
        .arg("mutants")
        .args([
            "--list",
            "--re",
            r"^src/simple_fns.rs:\d+:\d+: replace returns_unit with \(\)$",
        ])
        .arg("-d")
        .arg(tmp.path())
        .assert()
        .success()
        .stdout("src/simple_fns.rs:8:5: replace returns_unit with ()\n");
}

#[test]
fn list_mutants_regex_filters_json() {
    let tmp = copy_of_testdata("well_tested");
    run()
        .arg("mutants")
        .args([
            "--list",
            "--re",
            "divisible",
            "--exclude-re",
            "false",
            "--json",
        ])
        .arg("-d")
        .arg(tmp.path())
        .assert_insta("list_mutants_regex_filters_json");
}

#[test]
fn list_mutants_well_tested_multiple_examine_and_exclude_name_filter_with_files_and_folders() {
    let tmp = copy_of_testdata("with_child_directories");
    run()
        .arg("mutants")
        .args(["--list", "--file", "module_methods.rs", "--file", "utils", "--exclude", "**/sub_utils/**", "--exclude", "nested_function.rs"])
        .current_dir(tmp.path())
        .assert_insta("list_mutants_well_tested_multiple_examine_and_exclude_name_filter_with_files_and_folders");
}

#[test]
fn list_mutants_json_well_tested() {
    let tmp = copy_of_testdata("well_tested");
    run()
        .arg("mutants")
        .arg("--list")
        .arg("--json")
        .current_dir(tmp.path())
        .assert_insta("list_mutants_json_well_tested");
}

#[test]
fn list_files_text_well_tested() {
    let tmp = copy_of_testdata("well_tested");
    run()
        .arg("mutants")
        .arg("--list-files")
        .current_dir(tmp.path())
        .assert_insta("list_files_text_well_tested");
}

#[test]
fn list_files_respects_file_filters() {
    // Files matching excludes *are* visited to find references to other modules,
    // but they're not included in --list-files.
    let tmp = copy_of_testdata("well_tested");
    run()
        .arg("mutants")
        .args(["--list-files", "--exclude", "lib.rs"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("methods.rs"))
        .stdout(predicate::str::contains("lib.rs").not());
}

#[test]
fn list_files_json_well_tested() {
    let tmp = copy_of_testdata("well_tested");
    run()
        .arg("mutants")
        .arg("--list-files")
        .arg("--json")
        .current_dir(tmp.path())
        .assert_insta("list_files_json_well_tested");
}

#[test]
fn no_mutants_in_tree_everything_skipped() {
    let tmp_src_dir = copy_of_testdata("everything_skipped");
    run()
        .args(["mutants", "--list"])
        .arg("--dir")
        .arg(tmp_src_dir.path())
        .assert()
        .stderr(predicate::str::is_empty()) // not an error or warning
        .stdout(predicate::str::is_empty())
        .success();
}

#[test]
fn list_mutants_with_alternate_registry() {
    // For https://github.com/sourcefrog/cargo-mutants/issues/428

    // This tree has a non-default registry that ends up just pointing back to crates.io, but under another name.
    //
    // Without running cargo metadata properly this will fail.
    //
    // The tree doesn't actually generate any mutants.
    //
    // To reproduce the failure it's important that we *don't* run from the tree's directory.
    let tmp = copy_of_testdata("alternate_registry");
    run()
        .arg("mutants")
        .arg("--list")
        .arg("--dir")
        .arg(tmp.path())
        .assert()
        .stdout("")
        .success();
}

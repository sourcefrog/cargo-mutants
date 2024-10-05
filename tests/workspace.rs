// Copyright 2023-2024 Martin Pool

//! Tests for cargo workspaces with multiple packages.

use std::fs::{self, read_to_string};

use insta::assert_snapshot;
use itertools::Itertools;
use serde_json::json;

mod util;
use util::{assert_bytes_eq_json, copy_of_testdata, run};

#[test]
fn open_by_manifest_path() {
    let tmp = copy_of_testdata("factorial");
    run()
        .args(["mutants", "--list", "--line-col=false", "--manifest-path"])
        .arg(tmp.path().join("Cargo.toml"))
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "src/bin/factorial.rs: replace main with ()",
        ));
}

#[test]
fn list_warns_about_unmatched_packages() {
    run()
        .args([
            "mutants",
            "--list",
            "-d",
            "testdata/workspace",
            "-p",
            "notapackage",
        ])
        .assert()
        .stderr(predicates::str::contains(
            "package \"notapackage\" not found in source tree",
        ))
        .code(0);
}

#[test]
fn list_files_json_workspace() {
    // Demonstrates that we get package names in the json listing.
    let cmd = run()
        .args(["mutants", "--list-files", "--json"])
        .current_dir("testdata/workspace")
        .assert()
        .success();
    assert_bytes_eq_json(
        &cmd.get_output().stdout,
        json! {
        [
          {
            "package": "cargo_mutants_testdata_workspace_utils",
            "path": "utils/src/lib.rs"
          },
          {
            "package": "main",
            "path": "main/src/main.rs"
          },
          {
            "package": "main2",
            "path": "main2/src/main.rs"
          }
        ]
        },
    );
}

#[test]
fn list_files_as_json_in_workspace_subdir() {
    let cmd = run()
        .args(["mutants", "--list-files", "--json", "--workspace"])
        .current_dir("testdata/workspace/main2")
        .assert()
        .success();
    assert_bytes_eq_json(
        &cmd.get_output().stdout,
        json! {
            [
              {
                "package": "cargo_mutants_testdata_workspace_utils",
                "path": "utils/src/lib.rs"
              },
              {
                "package": "main",
                "path": "main/src/main.rs"
              },
              {
                "package": "main2",
                "path": "main2/src/main.rs"
              }
            ]
        },
    );
}

#[test]
fn workspace_tree_is_well_tested() {
    let tmp_src_dir = copy_of_testdata("workspace");
    run()
        .args(["mutants", "-d"])
        .arg(tmp_src_dir.path())
        .assert()
        .success();
    // The outcomes.json has some summary data
    let json_str =
        fs::read_to_string(tmp_src_dir.path().join("mutants.out/outcomes.json")).unwrap();
    println!("outcomes.json:\n{json_str}");
    let json: serde_json::Value = json_str.parse().unwrap();
    let total = json["total_mutants"].as_u64().unwrap();
    assert!(total > 8);
    assert_eq!(json["caught"].as_u64().unwrap(), total);
    assert_eq!(json["missed"].as_u64().unwrap(), 0);
    assert_eq!(json["timeout"].as_u64().unwrap(), 0);
    let outcomes = json["outcomes"].as_array().unwrap();

    {
        let baseline = outcomes[0].as_object().unwrap();
        assert_eq!(baseline["scenario"].as_str().unwrap(), "Baseline");
        assert_eq!(baseline["summary"], "Success");
        let baseline_phases = baseline["phase_results"].as_array().unwrap();
        assert_eq!(baseline_phases.len(), 2);
        assert_eq!(baseline_phases[0]["process_status"], "Success");
        assert_eq!(
            baseline_phases[0]["argv"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).skip(1).collect_vec().join(" "),
            "test --no-run --verbose --package cargo_mutants_testdata_workspace_utils --package main --package main2"
        );
        assert_eq!(baseline_phases[1]["process_status"], "Success");
        assert_eq!(
            baseline_phases[1]["argv"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .skip(1)
                .collect_vec()
                .join(" "),
            "test --verbose --package cargo_mutants_testdata_workspace_utils --package main --package main2"
        );
    }

    assert!(outcomes.len() > 9);
    for outcome in &outcomes[1..] {
        let mutant = &outcome["scenario"]["Mutant"];
        let package_name = mutant["package"].as_str().unwrap();
        assert!(!package_name.is_empty());
        assert_eq!(outcome["summary"], "CaughtMutant");
        let mutant_phases = outcome["phase_results"].as_array().unwrap();
        assert_eq!(mutant_phases.len(), 2);
        assert_eq!(mutant_phases[0]["process_status"], "Success");
        assert_eq!(
            mutant_phases[0]["argv"].as_array().unwrap()[1..=2],
            ["test", "--no-run"]
        );
        assert_eq!(mutant_phases[1]["process_status"], json!({"Failure": 101}));
        assert_eq!(
            mutant_phases[1]["argv"].as_array().unwrap()[1..=3],
            ["test", "--verbose", "--manifest-path"],
        );
    }
    {
        let baseline = json["outcomes"][0].as_object().unwrap();
        assert_eq!(baseline["scenario"].as_str().unwrap(), "Baseline");
        assert_eq!(baseline["summary"], "Success");
        let baseline_phases = baseline["phase_results"].as_array().unwrap();
        assert_eq!(baseline_phases.len(), 2);
        assert_eq!(baseline_phases[0]["process_status"], "Success");
        assert_eq!(
            baseline_phases[0]["argv"].as_array().unwrap()[1..].iter().map(|v| v.as_str().unwrap()).join(" "),
            "test --no-run --verbose --package cargo_mutants_testdata_workspace_utils --package main --package main2",
        );
        assert_eq!(baseline_phases[1]["process_status"], "Success");
        assert_eq!(
            baseline_phases[1]["argv"].as_array().unwrap()[1..]
                .iter()
                .map(|v| v.as_str().unwrap())
                .join(" "),
            "test --verbose --package cargo_mutants_testdata_workspace_utils --package main --package main2",
        );
    }
}

#[test]
/// Baseline tests in a workspace only test the packages that will later
/// be mutated.
/// See <https://github.com/sourcefrog/cargo-mutants/issues/151>
fn in_workspace_only_relevant_packages_included_in_baseline_tests_by_file_filter() {
    let tmp = copy_of_testdata("package_fails");
    run()
        .args(["mutants", "-f", "passing/src/lib.rs", "--no-shuffle", "-d"])
        .arg(tmp.path())
        .assert()
        .success();
    assert_snapshot!(
        read_to_string(tmp.path().join("mutants.out/caught.txt")).unwrap(),
        @r###"
    passing/src/lib.rs:2:5: replace triple -> usize with 0
    passing/src/lib.rs:2:5: replace triple -> usize with 1
    passing/src/lib.rs:2:7: replace * with + in triple
    passing/src/lib.rs:2:7: replace * with / in triple
    "###);
    assert_eq!(
        read_to_string(tmp.path().join("mutants.out/timeout.txt")).unwrap(),
        ""
    );
    assert_eq!(
        read_to_string(tmp.path().join("mutants.out/missed.txt")).unwrap(),
        ""
    );
    assert_eq!(
        read_to_string(tmp.path().join("mutants.out/unviable.txt")).unwrap(),
        ""
    );
}

/// Even the baseline test only tests the explicitly selected packages,
/// so it doesn't fail if some packages don't build.
#[test]
fn baseline_test_respects_package_options() {
    let tmp = copy_of_testdata("package_fails");
    run()
        .args([
            "mutants",
            "--package",
            "cargo-mutants-testdata-package-fails-passing",
            "--no-shuffle",
            "-d",
        ])
        .arg(tmp.path())
        .assert()
        .success();
    assert_snapshot!(
        read_to_string(tmp.path().join("mutants.out/caught.txt")).unwrap(),
        @r###"
    passing/src/lib.rs:2:5: replace triple -> usize with 0
    passing/src/lib.rs:2:5: replace triple -> usize with 1
    passing/src/lib.rs:2:7: replace * with + in triple
    passing/src/lib.rs:2:7: replace * with / in triple
    "###
    );
    assert_eq!(
        read_to_string(tmp.path().join("mutants.out/timeout.txt")).unwrap(),
        ""
    );
    assert_eq!(
        read_to_string(tmp.path().join("mutants.out/missed.txt")).unwrap(),
        ""
    );
    assert_eq!(
        read_to_string(tmp.path().join("mutants.out/unviable.txt")).unwrap(),
        ""
    );
}

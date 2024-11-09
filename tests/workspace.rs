// Copyright 2023-2024 Martin Pool

//! Tests for cargo workspaces with multiple packages.

use std::fs::{self, create_dir, read_to_string, write};

use insta::assert_snapshot;
use itertools::Itertools;
use predicates::prelude::predicate;
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
    let tmp = copy_of_testdata("workspace");
    let cmd = run()
        .args(["mutants", "--list-files", "--json"])
        .current_dir(tmp.path())
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
    let tmp = copy_of_testdata("workspace");
    let cmd = run()
        .args(["mutants", "--list-files", "--json", "--workspace"])
        .current_dir(tmp.path().join("main2"))
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
            mutant_phases[1]["argv"].as_array().unwrap()[1..=2],
            ["test", "--verbose"],
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

#[test]
fn cross_package_tests() {
    // This workspace has two packages, one of which contains the tests.
    // Mutating the one with no tests will find test gaps, but
    // either testing the whole workspace, or naming the test package,
    // will show that it's actually all well tested.
    //
    // <https://github.com/sourcefrog/cargo-mutants/issues/394>

    let tmp = copy_of_testdata("cross_package_tests");
    let path = tmp.path();

    // Testing only this one package will find gaps.
    run()
        .args(["mutants"])
        .arg("-d")
        .arg(path.join("lib"))
        .assert()
        .stdout(predicate::str::contains("4 missed"))
        .code(2); // missed mutants

    // Just asking to *mutate* the whole workspace will not cause us
    // to run the tests in "tests" against mutants in "lib".
    run()
        .args(["mutants", "--workspace"])
        .arg("-d")
        .arg(path.join("lib"))
        .assert()
        .stdout(predicate::str::contains("4 missed"))
        .code(2); // missed mutants

    // Similarly, starting in the workspace dir is not enough.
    run()
        .args(["mutants"])
        .arg("-d")
        .arg(path)
        .assert()
        .stdout(predicate::str::contains("4 missed"))
        .code(2); // missed mutants

    // Testing the whole workspace does catch everything.
    run()
        .args(["mutants", "--test-workspace=true"])
        .arg("-d")
        .arg(path.join("lib"))
        .assert()
        .stdout(predicate::str::contains("4 caught"))
        .code(0);

    // And naming the test package also catches everything.
    run()
        .args([
            "mutants",
            "--test-package=cargo-mutants-testdata-cross-package-tests-tests",
        ])
        .arg("-d")
        .arg(path.join("lib"))
        .assert()
        .stdout(predicate::str::contains("4 caught"))
        .code(0);

    // Using the wrong package name is an error
    run()
        .args(["mutants", "--test-package=tests"])
        .arg("-d")
        .arg(path.join("lib"))
        .env_remove("RUST_BACKTRACE")
        .assert()
        .stderr(
            "Error: Some package names in --test-package are not present in the workspace: tests\n",
        )
        .code(1);

    // You can configure the test package in the workspace
    let cargo_dir = path.join(".cargo");
    create_dir(&cargo_dir).unwrap();
    let mutants_toml_path = cargo_dir.join("mutants.toml");
    write(&mutants_toml_path, b"test_workspace = true").unwrap();
    // Now the mutants are caught
    run()
        .args(["mutants"])
        .arg("-d")
        .arg(path.join("lib"))
        .assert()
        .stdout(predicate::str::contains("4 caught"))
        .code(0);

    // It would also work to name the test package
    write(
        &mutants_toml_path,
        br#"test_package = ["cargo-mutants-testdata-cross-package-tests-tests"]"#,
    )
    .unwrap();
    run()
        .args(["mutants"])
        .arg("-d")
        .arg(path.join("lib"))
        .assert()
        .stdout(predicate::str::contains("4 caught"))
        .code(0);
}

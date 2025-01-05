// Copyright 2022 - 2025 Martin Pool.

//! Test handling of `--jobs` concurrency option.

use std::fs::read_to_string;

use itertools::Itertools;
use regex::Regex;

mod util;
use util::{copy_of_testdata, run};

/// It's a bit hard to assess that multiple jobs really ran in parallel,
/// but we can at least check that the option is accepted, and see that the
/// debug log looks like it's using multiple threads.
#[test]
fn jobs_option_accepted_and_causes_multiple_threads() {
    let testdata = copy_of_testdata("small_well_tested");
    run()
        .arg("mutants")
        .arg("-d")
        .arg(testdata.path())
        .arg("-j2")
        .arg("--minimum-test-timeout=120") // to avoid flakes on slow CI
        .assert()
        .success();
    let debug_log =
        read_to_string(testdata.path().join("mutants.out/debug.log")).expect("read debug log");
    println!("debug log:\n{debug_log}");
    // This might be brittle, as the ThreadId debug form is not specified, and
    // also _possibly_ everything will complete on one thread before the next
    // gets going, though that seems unlikely.
    let re = Regex::new(r#"start thread thread_id=ThreadId\(\d+\)"#).expect("compile regex");
    let matches = re
        .find_iter(&debug_log)
        .map(|m| m.as_str())
        .unique()
        .collect::<Vec<_>>();
    println!("threadid matches: {matches:?}");
    assert!(
        matches.len() > 1,
        "expected more than {} thread ids in debug log",
        matches.len()
    );
}

#[test]
fn warn_about_too_many_jobs() {
    let testdata = copy_of_testdata("small_well_tested");
    run()
        .arg("mutants")
        .arg("-d")
        .arg(testdata.path())
        .arg("-j40")
        .arg("--shard=0/1000")
        .assert()
        .stderr(predicates::str::contains(
            "WARN --jobs=40 is probably too high",
        ))
        .success();
}

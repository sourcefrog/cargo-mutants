// Copyright 2022-2023 Martin Pool.

//! Test handling of `--jobs` concurrency option.

use super::{copy_of_testdata, run};

/// It's a bit hard to assess that multiple jobs really ran in parallel,
/// but we can at least check that the option is accepted.
#[test]
fn jobs_option_accepted() {
    let testdata = copy_of_testdata("well_tested");
    run()
        .arg("mutants")
        .arg("-d")
        .arg(testdata.path())
        .arg("-j3")
        .arg("--minimum-test-timeout=120") // to avoid flakes on slow CI
        .assert()
        .success();
}

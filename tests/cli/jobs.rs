// Copyright 2022-2023 Martin Pool.

//! Test handling of `--jobs` concurrency option.

use super::{copy_of_testdata, run};

/// It's a bit hard to assess that multiple jobs really ran in parallel,
/// but we can at least check that the option is accepted, and see that the
/// debug log looks like it's using multiple threads.
#[test]
fn jobs_option_accepted() {
    let testdata = copy_of_testdata("small_well_tested");
    run()
        .arg("mutants")
        .arg("-d")
        .arg(testdata.path())
        .arg("-j2")
        .arg("--minimum-test-timeout=120") // to avoid flakes on slow CI
        .assert()
        .success();
}

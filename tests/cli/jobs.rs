// Copyright 2022 Martin Pool.

//! Test handling of `-j` option.

use super::{copy_of_testdata, run_assert_cmd};

/// It's a bit hard to assess that multiple jobs really ran in parallel,
/// but we can at least check that the option is accepted.
#[test]
fn jobs_option_accepted() {
    let testdata = copy_of_testdata("well_tested");
    run_assert_cmd()
        .arg("mutants")
        .arg("-d")
        .arg(testdata.path())
        .arg("-j3")
        .assert()
        .success();
}

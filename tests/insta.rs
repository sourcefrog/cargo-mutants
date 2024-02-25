// Copyright 2021-2024 Martin Pool

//! Test special handling of cargo-insta.

mod util;
use util::{copy_of_testdata, run};

/// `INSTA_UPDATE=always` in the environment will cause Insta to update
/// the snaphots, so the tests will pass, so mutants will not be caught.
/// This test checks that cargo-mutants sets the environment variable
/// so that mutants are caught properly.
#[test]
fn insta_test_failures_are_detected() {
    for insta_update in ["auto", "always"] {
        println!("INSTA_UPDATE={insta_update}");
        let tmp_src_dir = copy_of_testdata("insta");
        run()
            .arg("mutants")
            .args(["--no-times", "--no-shuffle", "--caught"])
            .env("INSTA_UPDATE", insta_update)
            .current_dir(tmp_src_dir.path())
            .assert()
            .success();
    }
}

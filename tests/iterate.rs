// Copyright 2024 Martin Pool

//! Tests for `--iterate`

mod util;

use std::fs::{create_dir, read_to_string, write, File};
use std::io::Write;

use indoc::indoc;
use predicates::prelude::*;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

use self::util::run;

#[test]
fn iterate() {
    let temp = tempdir().unwrap();

    write(
        temp.path().join("Cargo.toml"),
        indoc! { r#"
            [package]
            name = "cargo_mutants_iterate"
            edition = "2021"
            version = "0.0.0"
            publish = false
        "# },
    )
    .unwrap();
    create_dir(temp.path().join("src")).unwrap();
    create_dir(temp.path().join("tests")).unwrap();

    // First, write some untested code, and expect that the mutant is missed.
    write(
        temp.path().join("src/lib.rs"),
        indoc! { r#"
            pub fn is_two(a: usize) -> bool { a == 2 }
        "#},
    )
    .unwrap();

    run()
        .arg("mutants")
        .arg("-d")
        .arg(temp.path())
        .arg("--list")
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(indoc! { r#"
            src/lib.rs:1:35: replace is_two -> bool with true
            src/lib.rs:1:35: replace is_two -> bool with false
            src/lib.rs:1:37: replace == with != in is_two
        "# });

    run()
        .arg("mutants")
        .arg("--no-shuffle")
        .arg("-d")
        .arg(temp.path())
        .assert()
        .code(2); // missed mutants

    assert_eq!(
        read_to_string(temp.path().join("mutants.out/missed.txt")).unwrap(),
        indoc! { r#"
            src/lib.rs:1:35: replace is_two -> bool with true
            src/lib.rs:1:35: replace is_two -> bool with false
            src/lib.rs:1:37: replace == with != in is_two
        "# }
    );
    assert_eq!(
        read_to_string(temp.path().join("mutants.out/caught.txt")).unwrap(),
        ""
    );
    assert!(!temp
        .path()
        .join("mutants.out/previously_caught.txt")
        .is_file());

    // Now add a test that should catch this.
    write(
        temp.path().join("tests/main.rs"),
        indoc! { r#"
        use cargo_mutants_iterate::*;

        #[test]
        fn some_test() {
            assert!(is_two(2));
            assert!(!is_two(4));
        }
    "#},
    )
    .unwrap();

    run()
        .arg("mutants")
        .arg("--no-shuffle")
        .arg("-d")
        .arg(temp.path())
        .assert()
        .code(0); // caught it

    assert_eq!(
        read_to_string(temp.path().join("mutants.out/missed.txt")).unwrap(),
        ""
    );
    assert_eq!(
        read_to_string(temp.path().join("mutants.out/caught.txt")).unwrap(),
        indoc! { r#"
            src/lib.rs:1:35: replace is_two -> bool with true
            src/lib.rs:1:35: replace is_two -> bool with false
            src/lib.rs:1:37: replace == with != in is_two
        "# }
    );

    // Now that everything's caught, run tests again and there should be nothing to test,
    // on both the first and second run with --iterate
    for _ in 0..2 {
        run()
            .arg("mutants")
            .args(["--list", "--iterate"])
            .arg("-d")
            .arg(temp.path())
            .assert()
            .success()
            .stdout("");
        run()
            .arg("mutants")
            .args(["--no-shuffle", "--iterate", "--in-place"])
            .arg("-d")
            .arg(temp.path())
            .assert()
            .success()
            .stderr(predicate::str::contains(
                "No mutants found under the active filters",
            ))
            .stdout(predicate::str::contains("Found 0 mutants to test"));
        assert_eq!(
            read_to_string(temp.path().join("mutants.out/caught.txt")).unwrap(),
            ""
        );
        assert_eq!(
            read_to_string(temp.path().join("mutants.out/previously_caught.txt"))
                .unwrap()
                .lines()
                .count(),
            3
        );
    }

    // Add some more code and it should be seen as untested.
    let mut src = File::options()
        .append(true)
        .open(temp.path().join("src/lib.rs"))
        .unwrap();
    src.write_all("pub fn not_two(a: usize) -> bool { !is_two(a) }\n".as_bytes())
        .unwrap();
    drop(src);

    // We should see only the new function as untested
    let added_mutants = indoc! { r#"
        src/lib.rs:2:36: replace not_two -> bool with true
        src/lib.rs:2:36: replace not_two -> bool with false
        src/lib.rs:2:36: delete ! in not_two
    "# };
    run()
        .arg("mutants")
        .args(["--list", "--iterate"])
        .arg("-d")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(added_mutants);

    // These are missed by a new incremental run
    run()
        .arg("mutants")
        .args(["--no-shuffle", "--iterate", "--in-place"])
        .arg("-d")
        .arg(temp.path())
        .assert()
        .code(2);
    assert_eq!(
        read_to_string(temp.path().join("mutants.out/missed.txt")).unwrap(),
        added_mutants
    );

    // Add a new test that catches some but not all mutants
    File::options()
        .append(true)
        .open(temp.path().join("tests/main.rs"))
        .unwrap()
        .write_all("#[test] fn three_is_not_two() { assert!(not_two(3)); }\n".as_bytes())
        .unwrap();
    run()
        .arg("mutants")
        .args(["--no-shuffle", "--iterate", "--in-place"])
        .arg("-d")
        .arg(temp.path())
        .assert()
        .code(2);
    assert_eq!(
        read_to_string(temp.path().join("mutants.out/missed.txt")).unwrap(),
        "src/lib.rs:2:36: replace not_two -> bool with true\n"
    );

    // There should only be one more mutant to test
    run()
        .arg("mutants")
        .args(["--list", "--iterate", "--in-place"])
        .arg("-d")
        .arg(temp.path())
        .assert()
        .success()
        .stdout("src/lib.rs:2:36: replace not_two -> bool with true\n");

    // Add another test
    File::options()
        .append(true)
        .open(temp.path().join("tests/main.rs"))
        .unwrap()
        .write_all("#[test] fn two_is_not_not_two() { assert!(!not_two(2)); }\n".as_bytes())
        .unwrap();
    run()
        .arg("mutants")
        .args(["--list", "--iterate", "--in-place"])
        .arg("-d")
        .arg(temp.path())
        .assert()
        .success()
        .stdout("src/lib.rs:2:36: replace not_two -> bool with true\n");
    run()
        .arg("mutants")
        .args(["--no-shuffle", "--iterate", "--in-place"])
        .arg("-d")
        .arg(temp.path())
        .assert()
        .success();
    assert_eq!(
        read_to_string(temp.path().join("mutants.out/missed.txt")).unwrap(),
        ""
    );
    assert_eq!(
        read_to_string(temp.path().join("mutants.out/caught.txt")).unwrap(),
        "src/lib.rs:2:36: replace not_two -> bool with true\n"
    );
    assert_eq!(
        read_to_string(temp.path().join("mutants.out/previously_caught.txt"))
            .unwrap()
            .lines()
            .count(),
        5
    );

    // nothing more is missed
    run()
        .arg("mutants")
        .args(["--list", "--iterate", "--in-place"])
        .arg("-d")
        .arg(temp.path())
        .assert()
        .success()
        .stdout("");
}

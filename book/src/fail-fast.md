# Fail-fast tests

When cargo-mutants runs the test suite, it only needs to find out if any tests fail, and so it's desirable that the test suite stop on the first failure.

In `cargo test` there are separate fail-fast configurations at two levels: the whole target and the individual test.

By default, `cargo test` will stop running test targets after the first one fails. Do not pass `--no-fail-fast` to `cargo test` under cargo-mutants.

Rust nightly releases after 1.92.0-nightly (2025-09-18) accept a `--fail-fast` option to the test harness. (This is distinct from the `--fail-fast` option to `cargo test`.) This causes the test target to stop after the first individual test fails. This can significantly improve cargo-mutants performance in the common case where there are many tests in a single target or some of them are slow.

If you have a sufficiently recent toolchain you can enable this in the [`cargo_test_args`](cargo-args.md):

    cargo mutants -- -- -Zunstable-options --fail-fast

*Note*: There are two `--` separators: the first delimits the arguments from `cargo mutants` to be passed to `cargo test` and the second delimits the arguments from `cargo test` so they are passed to the test target.

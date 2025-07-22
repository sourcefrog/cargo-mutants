# cargo-mutants with nextest

[nextest](https://nexte.st) is a tool for running Rust tests, as a replacement for `cargo test`.

You can use nextest to run your tests with cargo-mutants, instead of `cargo test`, by either passing the `--test-tool=nextest` option, or setting `test_tool = "nextest"` in `.cargo/mutants.toml`.

## Controlling nextest

You can pass additional arguments to nextest through the same [options and configuration keys as for Cargo](cargo-args.md).

For example, to select a [nextest profile](https://nexte.st/docs/configuration/?h=profile#profiles) (which is separate from a Cargo build profile):

    cargo mutants --cargo-arg=--profile=mutants

## How nextest works

In the context of cargo-mutants the most important difference between cargo-test and nextest is that nextest runs each test in a separate process, and it can run tests from multiple test targets in parallel. (Nextest also has some nice UI improvements and other features, but they're not relevant here.)

This means that nextest can stop faster if a single test fails, whereas cargo test will continue running all the tests within the test binary.

This is beneficial for cargo-mutants, because it only needs to know whether at least one test caught the mutation, and so exiting as soon as there's a failure is better.

However, [nextest currently allows straggling tests to run to completion](https://github.com/nextest-rs/nextest/discussions/2482), even when one test has already failed. In a tree with fast unit tests and slow integration tests this can mean that nextest is actually slower than the default test runner.

## When to use nextest

There are at least two reasons why you might want to use nextest:

1. Some Rust source trees only support testing under nextest, and their tests fail under `cargo test`: in that case, you have to use this option! In particular, nextest's behavior of running each test in a separate process gives better isolation between tests.

2. Some trees might be faster under nextest than under `cargo test`, because they have a lot of tests that fail quickly, and the startup time is a small fraction of the time for the average test. This may or may not be true for your tree, so you can try it and see. Some trees, including cargo-mutants itself, are slower under nextest.

## nextest and doctests

**Caution:** [nextest currently does not run doctests](https://github.com/nextest-rs/nextest/issues/16), so behaviors that are only caught by doctests will show as missed when using nextest. (cargo-mutants could separately run the doctests, but currently does not.)

# cargo-mutants with nextest

[nextest](https://nexte.st) is a tool for running Rust tests, as a replacement for `cargo test`.

You can use nextest to run your tests with cargo-mutants, instead of `cargo test`, by either passing the `--test-tool=nextest` option, or setting `test_tool = "nextest"` in `.cargo/mutants.toml`.

## How nextest works

In the context of cargo-mutants the most important difference between cargo-test and nextest is that nextest runs each test in a separate process, and it can run tests from multiple test targets in parallel. (Nextest also has some nice UI improvements and other features, but they're not relevant here.)

This means that nextest can stop faster if a single test fails, whereas cargo test will continue running all the tests within the test binary.

This is beneficial for cargo-mutants, because it only needs to know whether at least one test caught the mutation, and so exiting as soon as there's a failure is better.

However, running each test individually also means there is more per-test startup cost, and so on some trees nextest may be slower.

In general, nextest will do relatively poorly on trees that have tests that are individually very fast, or on trees that establish shared or cached state across tests.

## When to use nextest

There are at least two reasons why you might want to use nextest:

1. Some Rust source trees only support testing under nextest, and their tests fail under `cargo test`: in that case, you have to use this option! In particular, nextest's behavior of running each test in a separate process gives better isolation between tests.

2. Some trees might be faster under nextest than under `cargo test`, because they have a lot of tests that fail quickly, and the startup time is a small fraction of the time for the average test. This may or may not be true for your tree, so you can try it and see. Some trees, including cargo-mutants itself, are slower under nextest.

## nextest and doctests

**Caution:** [nextest currently does not run doctests](https://github.com/nextest-rs/nextest/issues/16), so behaviors that are only caught by doctests will show as missed when using nextest. (cargo-mutants could separately run the doctests, but currently does not.)

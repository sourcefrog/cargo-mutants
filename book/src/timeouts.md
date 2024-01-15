# Hangs and timeouts

Some mutations to the tree can cause the test suite to hang. For example, in
this code, cargo-mutants might try changing `should_stop` to always return
`false`, but this will cause the program to hang:

```rust
    while !should_stop() {
      // something
    }
```

In general you will want to skip functions which cause a hang when mutated,
either by [marking them with an attribute](skip.md) or in the [configuration
file](filter_mutants.md).

## Timeouts

To avoid hangs, cargo-mutants will kill the test suite after a timeout and
continue to the next mutant.

By default, the timeout is set automatically. cargo-mutants measures the time to
run the test suite in the unmodified tree, and then sets a timeout for mutated
tests at 5x the time to run tests with no mutations, and a minimum of 20
seconds.

The minimum of 20 seconds can be overridden by the
`CARGO_MUTANTS_MINIMUM_TEST_TIMEOUT` environment variable, measured in seconds.

You can also set an explicit timeout with the `--timeout` option, also measured
in seconds. If this option is specified then the timeout is also applied to the
unmutated tests.

The timeout does not apply to `cargo check` or `cargo build`, only `cargo test`.

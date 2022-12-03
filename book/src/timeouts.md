## Hangs and timeouts

Some mutations to the tree can cause the test suite to hang. For example, in
this code, cargo-mutants might try changing `should_stop` to always return
`false`:

```rust
    while !should_stop() {
      // something
    }
```

`cargo mutants` automatically sets a timeout when running tests with mutations
applied, and reports mutations that hit a timeout. The automatic timeout is the greater of
20 seconds, or 5x the time to run tests with no mutations.

The `CARGO_MUTANTS_MINIMUM_TEST_TIMEOUT` environment variable, measured in seconds, overrides the minimum time.

You can also set an explicit timeout with the `--timeout` option, also measure in seconds. In this case
the timeout is also applied to tests run with no mutation.

The timeout does not apply to `cargo check` or `cargo build`, only `cargo test`.

To make future runs faster, you can [skip mutations that hit a timeout](skip.md).

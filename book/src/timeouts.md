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

To avoid hangs, cargo-mutants will kill the build or test after a timeout and
continue to the next mutant.

By default, the timeouts are set automatically, relative to the times taken to
build and test the unmodified tree (baseline).

The default timeouts are:
- `cargo build`/`cargo check`: 2 times the baseline build time
- `cargo test`: 5 times baseline test time (with a minimum of 20 seconds)

The minimum of 20 seconds for the test timeout can be overridden by the
`--minimum-test-timeout` option or the `CARGO_MUTANTS_MINIMUM_TEST_TIMEOUT` 
environment variable, measured in seconds.

You can set explicit timeouts with the `--build-timeout`, and `--timeout`
options, also measured in seconds. If these options are specified then they 
are applied to the baseline build and test as well.

You can set timeout multipliers that are relative to the duration of the
baseline build or test with `--build-timeout-multiplier` and
`--timeout-multiplier`, respectively.  Additionally, these can be configured
with `build_timeout_multiplier` and `timeout_multiplier` in
`.cargo/mutants.toml` (e.g. `timeout-multiplier = 1.5`).  These options are only
applied if the baseline is not skipped and no corresponding
`--build-timeout`/`--timeout` option is specified, otherwise they are ignored.

## Exceptions

The multiplier timeout options cannot be used when the baseline is skipped
(`--baseline=skip`), or when the build is in-place (`--in-place`). If no 
explicit timeouts is provided in these cases, then a default of 300 seconds
will be used.

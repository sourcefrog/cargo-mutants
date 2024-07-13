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

The default test timeout is 5 times the baseline test time, with a minimum of 20 seconds.

The minimum of 20 seconds for the test timeout can be overridden by the
`--minimum-test-timeout` option or the `CARGO_MUTANTS_MINIMUM_TEST_TIMEOUT`
environment variable, measured in seconds.

You can set an explicit timeouts with the `--timeout` option, also measured in seconds.

You can also set the test timeout as a multiple of the duration of the baseline test, with the `--timeout-multiplier` option and the `timeout_multiplier` configuration key.
The multiplier only has an effect if the baseline is not skipped and if `--timeout` is not specified.

## Build timeouts

`const` expressions may be evaluated at compile time. In the same way that mutations can cause tests to hang, mutations to const code may potentially cause the compiler to enter an infinite loop.

rustc imposes a time limit on evaluation of const expressions. This is controlled by the `long_running_const_eval` lint, which by default will interrupt compilation: as a result the mutants will be seen as unviable.

If this lint is configured off in your program, or if you use the `--cap-lints=true` option to turn off all lints, then the compiler may hang when constant expressions are mutated.

In this case you can use the `--build-timeout` or `--build-timeout-multiplier` options, or their corresponding configuration keys, to impose a limit on overall build time. However, because build time can be quite variable there's some risk of this causing builds to be flaky, and so it's off by default.

You might also choose to skip mutants that can cause long-running const evaluation.

## Exceptions

The multiplier timeout options cannot be used when the baseline is skipped
(`--baseline=skip`), or when the build is in-place (`--in-place`). If no
explicit timeouts is provided in these cases, then there is no build timeout and the test timeout default of 300 seconds will be used.

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

When a timeout expires, the process is sent `SIGTERM` so that it can clean up, and then
`SIGKILL` if it has not exited within a short grace period. A test process that ignores
`SIGTERM`, or that has been stopped and so never receives it, therefore cannot stall the
whole run.

## Build timeouts

`const` expressions may be evaluated at compile time. In the same way that mutations can cause tests to hang, mutations to const code may potentially cause the compiler to enter an infinite loop.

rustc imposes a time limit on evaluation of const expressions. This is controlled by the `long_running_const_eval` lint, which by default will interrupt compilation: as a result the mutants will be seen as unviable.

If this lint is configured off in your program, or if you use the `--cap-lints=true` option to turn off all lints, then the compiler may hang when constant expressions are mutated.

In this case you can use the `--build-timeout` or `--build-timeout-multiplier` options, or their corresponding configuration keys, to impose a limit on overall build time. However, because build time can be quite variable there's some risk of this causing builds to be flaky, and so it's off by default.

You might also choose to skip mutants that can cause long-running const evaluation.

## Leftover processes

A test can leave processes running after it returns: a daemon it started, a helper it
forgot to wait for, or a test binary that was not reaped. Those processes keep running
— and keep allocating — while cargo-mutants moves on to the next mutant, so they can
exhaust the machine's memory in a window where no cargo phase is running at all.

To prevent this, cargo-mutants starts each cargo invocation as the leader of its own
process group, and sweeps that group after *every* phase, not only after a timeout.
Once the cargo process itself exits, anything left in the group is sent `SIGTERM`, given
a short grace period, and then `SIGKILL`ed. What was reaped is recorded in the
scenario's log, and the pids are shown at `--level=debug`.

This has no effect on how a mutant is classified; it only stops work from one scenario
leaking into the next. Windows has no process groups, and cargo-mutants does not yet use
job objects, so this sweep is Unix-only.

## Why a scenario died

A mutant caught because the kernel killed its tests looks, in the summary counts, exactly
like a mutant caught by a failing assertion. When there is more to say, cargo-mutants says
it in parentheses on the outcome line:

```
caught   src/server.rs:88:5: replace listen -> bool with false (test killed by SIGABRT; test left 1 stray process behind (SIGKILLed: 30411)) in 2s build + 9s test
```

Two things get reported this way: the signal that killed a phase's cargo process, if it
died by one, and anything the process group sweep had to clean up. The same information
is written to the scenario's log and, in `mutants.out/outcomes.json`, to a `sweep` field
on each phase result.

This does not change the caught / missed / unviable / timeout classification. It only
makes the reason visible.

## Exceptions

The multiplier timeout options cannot be used when the baseline is skipped
(`--baseline=skip`), or when the build is in-place (`--in-place`). If no
explicit timeouts is provided in these cases, then there is no build timeout and the test timeout default of 300 seconds will be used.

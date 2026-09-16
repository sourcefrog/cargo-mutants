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

## Memory limits

A timeout is not always enough. A mutant can turn a bounded loop into an unbounded
allocator — flipping `+=` to `-=` on a parser's cursor, say — and a test that grows at
hundreds of megabytes per second can exhaust the machine long before the test timeout
arrives. On a CI runner the usual result is that the whole VM is torn down, with no log
and no record of which mutants had been tested.

`--max-memory SIZE`, or the `max_memory` key in the configuration file, puts a ceiling on
each scenario's cargo process tree instead, so that the kernel stops the scenario rather
than the machine. Sizes may be plain byte counts, or carry a `K`, `M`, `G`, or `T`
suffix, which are binary multiples: `1K` is 1024 bytes. The smallest accepted value is
1M: unlike `--timeout=0`, `--max-memory=0` is not a way to turn the limit off, so it is
rejected rather than silently stopping every scenario.

```shell
cargo mutants --max-memory 8G
```

```toml
# .cargo/mutants.toml
max_memory = "8G"
```

The limit is off by default, and applies to every phase of every scenario, builds
included, so leave room for the compiler as well as for the tests.

It is enforced with `setrlimit(RLIMIT_AS)` on the cargo process, inherited by everything
it spawns. That limits *address space*, which is a much cruder proxy than resident
memory: allocators and rustc reserve far more address space than they ever make
resident, so a limit that would be comfortable as a resident-memory ceiling can fail
builds outright when applied this way. **Set it generously.**

Which mechanism is in use is reported at startup:

```
INFO Limiting each scenario to 8589934592 bytes of memory using setrlimit(RLIMIT_AS)
```

On macOS, `RLIMIT_AS` is accepted by the kernel and then ignored, so `--max-memory` has
no effect there; cargo-mutants warns and carries on. On any platform where it cannot be
applied at all, giving `--max-memory` is an error, reported before any mutant is tested,
rather than a run that quietly had no limit.

This option does not change how mutants are classified. A mutant whose tests are stopped
by the limit fails its tests and so is caught, in just the same way as one that panics.

## Exceptions

The multiplier timeout options cannot be used when the baseline is skipped
(`--baseline=skip`), or when the build is in-place (`--in-place`). If no
explicit timeouts is provided in these cases, then there is no build timeout and the test timeout default of 300 seconds will be used.

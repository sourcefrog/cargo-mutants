# Baseline tests

Normally, cargo-mutants builds and runs your tree in a temporary directory before applying any mutations. This makes sure that your tests are in fact all passing, including in the copy of the tree that cargo-mutants will mutate.

Baseline tests can be skipped by passing the `--baseline=skip` command line option. (There is no config option for this.)

<div class="warning">
If you use <code>--baseline=skip</code>, you must make sure that the tests are actually passing, otherwise the results of cargo-mutants will be meaningless. cargo-mutants will probably report that all or most mutations were caught, but the test failures were not because of the mutations.
</div>

## Performance effects

The performance gain from skipping the baseline is one run of the full test suite, plus one incremental build. When the baseline is run, its build is typically slow because it must do the initial build of the tree, but when it is skipped, the first mutant will have to do a full (rather than incremental) build instead.

This means that, in a run that tests many mutants, the relative performance gain from skipping the baseline will be relatively small. However, it may still be useful to skip baseline tests in some specific situations.

## Timeouts

Normally, cargo-mutants uses the baseline test to establish an appropriate `timeout` for the test suite. If you skip the baseline, you should set `--timeout` manually.

## Use cases for skipping baseline tests

`--baseline=skip` might be useful in these situations:

1. You are running cargo-mutants in a CI or build system that separately runs the tests before cargo-mutants. In this case, you can be confident that the tests are passing, and you can save time by skipping the baseline. In particular, if you are [sharding](shards.md) work in CI, this avoids running the baseline on each shard.

2. You're repeatedly running `cargo-mutants` with different options, without changing the source code, perhaps with different `--file` or `--exclude` options.

3. You're developing `cargo-mutants` itself, and running it repeatedly on a tree that doesn't change.

# Sharding

In addition to [running multiple jobs locally](parallelism.md), cargo-mutants can also run jobs on multiple machines, to get an overall result faster by using more CPU cores.

Each job tests a subset of mutants, selected by a shard. Shards are described as `k/n`, where `n` is the number of shards and `k` is the index of the shard, from 0 to `n-1`.

There is no runtime coordination between shards: they each independently discover the available mutants and then select a subset based on the `--shard` option.

When `--list` and `--shard` are used together, only the mutants in the selected shard are listed.

If any shard fails then that would indicate that some mutants were missed, or there was some other problem.

## Consistency across shards

**CAUTION:**
All shards must be run with the same arguments, and the same sharding denominator `n`, or the results will be meaningless, as they won't agree on how to divide the work.

Sharding can be combined with filters or shuffling, as long as the filters are set consistently in all shards. Sharding can also combine with `--in-diff`, again as long as all shards see the same diff.

## Setting up sharding

Your CI system or other tooling is responsible for launching multiple shards, and for collecting the results. You're responsible for choosing the number of shards (see below).

For example, in GitHub Actions, you could use a matrix job to run multiple shards:

```yaml
{{#include ../../examples/workflows/sharded.yml}}
```

Note that the number of shards is set to match the `/8` in the `--shard` argument.

## Skipping the baseline

[Sharding works with `--baseline=skip`](baseline.md), to avoid the cost of running the baseline on every shard. But, if you do this, then you must ensure that the tests suite is passing in the baseline, for example by checking it in a previous CI step.

## Sharding algorithm

The `--sharding` command line and config option controls the algorithm by which mutants are distributed across shards.

* `slice` (the default): The first `n / k` mutants are assigned to shard 0, and so on. Because each shard successively builds related versions of the code, incremental builds may be faster, particularly in trees with many packages.

* `round-robin`: Mutant `i` is assigned to shard `i % n`. This distributes the mutants evenly across shards and is likely to cause shards to finish at similar times.

## Performance of sharding

Each mutant does some constant upfront work:

* Any CI setup including starting the machine, getting a checkout, installing a Rust toolchain, and installing cargo-mutants
* An initial clean build of the code under test
* A baseline run of the unmutated code (unless this is skipped)

Then, for each mutant in its shard, it does an incremental build and runs all the tests.

Each shard runs the same number of mutants, +/-1. Typically this will mean they each take roughly the same amount of time, although it's possible that some shards are unlucky in drawing mutants that happen to take longer to test.

A rough model for the overall execution time for all of the shards, allowing for this work occurring in parallel, is

```raw
SHARD_STARTUP + (CLEAN_BUILD + TEST) + (N_TOTAL_MUTANTS / N_SHARDS) * (INCREMENTAL_BUILD + TEST)
```

The total cost in CPU seconds can be modelled as:

```raw
N_SHARDS * (SHARD_STARTUP + CLEAN_BUILD + TEST) + N_MUTANTS * (INCREMENTAL_BUILD + TEST)
```

As a result, if you use many shards the cost of the initial build will dominate, and the overall time will converge towards the time for a clean build, a baseline test, and the test of one mutant.

## Choosing a number of shards

Because there's some constant overhead for every shard there will be diminishing returns and increasing ineffiency if you use too many shards. (In the extreme cases where there are more shards than mutants, some of them will find they have nothing to do and immediately exit.)

As a rule of thumb, you should probably choose `n` such that each worker runs at least 10 mutants, and possibly much more. 8 to 32 shards might be a good place to start.

The optimal setting probably depends on how long your tree takes to build from zero and incrementally, how long the tests take to run, and the performance of your CI system.

If your CI system offers a choice of VM sizes you might experiment with using smaller or larger VMs and more or less shards: the optimal setting probably also depends on your tree's ability to exploit larger machines.

You should also think about cost and capacity constraints in your CI system, and the risk of starving out other users.

cargo-mutants has no internal scaling constraints to prevent you from setting `n` very large, if cost, efficiency and CI capacity are not a concern.

## Sampling mutants

An option like `--shard 1/100` can be used to run 1% of all the generated mutants for testing cargo-mutants, to get a sense of whether it works or to see how it performs on some tree.

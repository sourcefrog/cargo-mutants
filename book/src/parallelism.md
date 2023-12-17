# Parallelism

After the initial test of the unmutated tree, cargo-mutants can run multiple
builds and tests of the tree in parallel on a single machine. Separately, you can
[shard](shards.md) work across multiple machines.

**Caution:** `cargo build` and `cargo test` internally spawn many threads and processes and can be very resource hungry. Don't set `--jobs` too high, or your machine may thrash, run out of memory, or overheat.

## Background

Even though cargo builds, rustc, and Rust's test framework launch multiple
processes or threads, they typically spend some time waiting for straggler tasks, during which time some CPU cores are idle. For example, a cargo build commonly ends up waiting for a single-threaded linker for several seconds.

Running one or more build or test tasks in parallel can use up this otherwise wasted capacity.
This can give significant performance improvements, depending on the tree under test and the hardware resources available.

## Timeouts

Because tests may be slower with high parallelism, or may exhibit more variability in execution time, you may see some spurious timeouts, and you may need to set `--timeout` manually to allow enough safety margin. (User feedback on this is welcome.)

## Non-hermetic tests

If your test suite is non-hermetic -- for example, if it talks to an external database -- then running multiple jobs in parallel may cause test flakes. `cargo-mutants` is just running multiple copies of `cargo test` simultaneously: if that doesn't work in your tree, then you can't use this option.

## Choosing a job count

You should set the number of jobs very conservatively, starting at `-j2` or `-j3`.

Higher settings are only likely to be helpful on very large machines, perhaps with >100 cores and >256GB RAM.

Unlike with `make`, setting `-j` proportionally to the number of cores is unlikely to work out well, because so the Rust build and test tools already parallelize very aggressively.

The best setting will depend on many factors including the behavior of your
program's test suite, the amount of memory on your system, and your system's
behavior under high load. Ultimately you'll need to experiment to find the best setting.

To tune the number of jobs, you can watch `htop` or some similar program while the tests are running, to see whether cores are fully utilized or whether the system is running out of memory. On laptop or desktop machines you might also want to watch the temperature of the CPU.

## Interaction with `--test-threads`

The Rust test framework exposes a `--test-threads` option controlling how many threads run inside a test binary. cargo-mutants doesn't set this, but you can set it from the command line, along with other parameters to the test binary. You might need to set this if your test suite is non-hermetic with regard to global process state.

Limiting the number of threads inside a single test binary would tend to make that binary less resource-hungry, and so _might_ allow you to set a higher `-j` option.

Reducing the number of test threads to increase `-j`  seems unlikely to help performance in most trees.

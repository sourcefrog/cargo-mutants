# Parallelism

After the initial test of the unmutated tree, cargo-mutants can test multiple
mutants in parallel. This can give significant performance improvements,
depending on the tree under test and the hardware resources available.

Even though cargo builds, rustc, and Rust's test framework launch multiple
processes or threads, they typically can't use all available CPU cores all the
time, and many `cargo test` runs will end up using only one core waiting for the
last task to complete. Running multiple jobs in parallel makes use of resources
that would otherwise be idle.

By default, only one job is run at a time.

To run more, use the `--jobs` or `-j` option, or set the `CARGO_MUTANTS_JOBS`
environment variable.

Setting this higher than the number of CPU cores is unlikely to be helpful.

The best setting will depend on many factors including the behavior of your
program's test suite, the amount of memory on your system, and your system's
behavior under high thermal load.

`-j 4` may be a good starting point. Start there and watch memory and CPU usage,
and tune towards a setting where all cores are fully utilized without apparent
thrashing, memory exhaustion, or thermal issues.

Because tests may be slower with high parallelism, you may see some spurious
timeouts, and you may need to set `--timeout` manually to allow enough safety
margin.

# Parallelism

The `--jobs` or `-j` option allows to test multiple mutants in parallel, by spawning several Cargo processes. This can give 25-50% performance improvements, depending on the tree under test and the hardware resources available.

It's common that for some periods of its execution, a single Cargo build or test job can't use all the available CPU cores. Running multiple jobs in parallel makes use of resources that would otherwise be idle.

However, running many jobs simultaneously may also put high demands on the
system's RAM (by running more compile/link/test tasks simultaneously), IO
bandwidth, and cooling (by fully using all cores).

The best setting will depend on many factors including the behavior of your program's test suite, the amount of memory on your system, and your system's behavior under high thermal load.

The default is currently to run only one job at a time. Setting this higher than the number of CPU cores is unlikely to be helpful.

`-j 4` may be a good starting point, even if you have many more CPU cores. Start
there and watch memory and CPU usage, and tune towards a setting where all cores
are always utilized without memory usage going too high, and without thermal
issues.

Because tests may be slower with high parallelism, you may see some spurious timeouts, and you may need to set `--timeout` manually to allow enough safety margin.

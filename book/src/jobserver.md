# Jobserver

The GNU Jobserver protocol enables a build system to limit the total number of concurrent jobs at any point in time.

By default, cargo-mutants starts a jobserver configured to allow one job per CPU. This limit applies across all the subprocesses spawned by cargo-mutants, including all parallel jobs. This allows you to use `--jobs` to run multiple test suites in parallel, without causing excessive load on the system from running too many compiler tasks in parallel.

`--jobserver=false` disables running the jobserver.

`--jobserver-tasks=N` sets the number of tasks that the jobserver will allow to run concurrently.

The Rust test framework does not currently use the jobserver protocol, so it won't affect tests, only builds. However, the jobserver can be observed by tests
and build scripts in `CARGO_MAKEFLAGS`.

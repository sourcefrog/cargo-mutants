# Improving performance

Most of the runtime for cargo-mutants is spent in running the program test suite
and in running incremental builds: both are done once per viable mutant.

So, anything you can do to make the `cargo build` and `cargo test` suite faster
will have a multiplicative effect on `cargo mutants` run time, and of course
will also make normal development more pleasant.

<https://matklad.github.io/2021/09/04/fast-rust-builds.html> has good general advice on making Rust builds and tests faster.

## Fail-fast tests

When cargo-mutants runs the test suite, it only needs to find out if any tests fail, and so it's desirable that the test suite stop on the first failure.

In `cargo test` there are separate fail-fast configurations at two levels: the whole target and the individual test.

By default, `cargo test` will stop running test targets after the first one fails. It is not advisable to pass `--no-fail-fast` to `cargo test` under cargo-mutants.

Rust nightly releases after 1.92.0-nightly (2025-09-18) accept a `--fail-fast` option to the test harness. (This is distinct from the `--fail-fast` option to `cargo test`.) This causes the test target to stop after the first individual test fails. This can significantly improve cargo-mutants performance in the common case where there are many tests in a single target or some of them are slow.

If you have a sufficiently recent toolchain you can enable this in the [`cargo_test_args`](cargo-args.md):

    cargo mutants -- -- -Zunstable-options --fail-fast

*Note*: There are two `--` separators: the first delimits the arguments from `cargo mutants` to be passed to `cargo test` and the second delimits the arguments from `cargo test` so they are passed to the test target.

## Avoid doctests

Rust doctests are pretty slow, because every doctest example becomes a separate
test binary. If you're using doctests only as testable documentation and not to
assert correctness of the code, you can skip them with `cargo mutants --
--all-targets`.

## Choosing a cargo profile

[Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html) provide a way to configure compiler settings including several that influence build and runtime performance.

By default, cargo-mutants will use the default profile selected for `cargo test`, which is also called `test`. This includes debug symbols but disables optimization.

You can select a different profile using the `--profile` option or the `profile` configuration key.

You may wish to define a `mutants` profile in `Cargo.toml`, such as:

```toml
[profile.mutants]
inherits = "test"
debug = "none"
```

and then configure this as the default in `.cargo/mutants.toml`:

```toml
profile = "mutants"
```

Turning off debug symbols will make the builds faster, at the expense of possibly giving less useful output when a test fails. In general, since mutants are expected to cause tests to fail, debug symbols may not be worth cost.

If your project's tests take a long time to run then it may be worth experimenting with increasing the `opt` level or other optimization parameters in the profile, to trade off longer builds for faster test runs.

cargo-mutants now shows the breakdown of build versus test time which may help you work out if this will help: if the tests are much slower than the build it's worth trying more more compiler optimizations.

## Ramdisks

cargo-mutants causes the Rust toolchain (and, often, the program under test) to read and write _many_ temporary files. Setting the temporary directory onto a ramdisk can improve performance significantly. This is particularly important with parallel builds, which might otherwise hit disk bandwidth limits.

See your OS's documentation for how to configure a ramdisk.

To temporarily configure a ramdisk on Linux as an experiment:

```shell
sudo mkdir /ram
sudo mount -t tmpfs /ram /ram
sudo chmod 1777 /ram
env TMPDIR=/ram cargo mutants
```

Some Rust build directories can be multiple gigabytes in size, and if you use `cargo mutants -j` there will be several directories of that size. Be careful that the ramdisk does not use so much memory that it causes the system to swap.

## Using faster linkers

Because cargo-mutants does many incremental builds, link time is important, especially if the test suite is relatively fast.

Using a non-default linker can give a significant performance improvement. The exact amount will depend on the project.

Using the [Mold linker](https://github.com/rui314/mold) on Unix can give a 20% performance improvement, depending on the tree.

On Linux, the [Wild linker](https://github.com/davidlattimore/wild) can give a significant performance improvement, potentially even better than Mold. On one tree, using Wild cut the time to run cargo-mutants by more than half.

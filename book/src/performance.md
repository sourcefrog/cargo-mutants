# Improving performance

Most of the runtime for cargo-mutants is spent in running the program test suite
and in running incremental builds: both are done once per viable mutant.

So, anything you can do to make the `cargo build` and `cargo test` suite faster
will have a multiplicative effect on `cargo mutants` run time, and of course
will also make normal development more pleasant.

<https://matklad.github.io/2021/09/04/fast-rust-builds.html> has good general advice on making Rust builds and tests faster.

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

Because of limitations in the way cargo-mutants runs Cargo, the standard way of configuring Mold for Rust in `~/.cargo/config.toml` won't work.

Instead, set the `RUSTFLAGS` environment variable to `-Clink-arg=-fuse-ld=mold`.

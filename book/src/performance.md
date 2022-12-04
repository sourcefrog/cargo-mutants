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

## Optimized builds

On _some but not all_ projects, cargo-mutants can be faster if you use `-C --release`, which will make the build slower but may make the tests faster. Typically this will help on projects with very long CPU-intensive test suites.

cargo-mutants now shows the breakdown of build versus test time which may help you work out if this will help: if the tests are much slower than the build it's worth trying more more compiler optimizations.

On projects like this you might also choose just to turn up optimization for all debug builds in [`.cargo/config.toml`](https://doc.rust-lang.org/cargo/reference/config.html).

## Ramdisks

cargo-mutants causes the Rust toolchain (and, often, the program under test) to read and write _many_ temporary files. Setting the temporary directory onto a ramdisk can improve performance significantly. This is particularly important with parallel builds, which might otherwise hit disk bandwidth limits. For example on Linux:

```shell
sudo mkdir /ram
sudo mount -t tmpfs /ram /ram  # or put this in fstab, or just change /tmp
env TMPDIR=/ram cargo mutants
```

## Using the Mold linker

Using the [Mold linker](https://github.com/rui314/mold) on Unix can give a 20% performance improvement, depending on the tree.
Because cargo-mutants does many
incremental builds, link time is important, especially if the test suite is relatively fast.

Because of limitations in the way cargo-mutants runs Cargo, the standard way of configuring Mold for Rust in `~/.cargo/config.toml` won't work.

Instead, set the `RUSTFLAGS` environment variable to `-Clink-arg=-fuse-ld=mold`.

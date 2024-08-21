# Installation

Install cargo-mutants from source:

```sh
cargo install --locked cargo-mutants
```

You can also use `cargo binstall` from [cargo-binstall](https://github.com/cargo-bins/cargo-binstall), or install binaries from GitHub releases.

## Supported Rust versions

Building cargo-mutants requires a reasonably recent stable (or nightly or beta) Rust toolchain.
The supported version is visible in <https://crates.io/crates/cargo-mutants>.

After installing cargo-mutants, you should be able to use it to run tests under
any toolchain, even toolchains that are far too old to build cargo-mutants, using the standard `+` option to `cargo`:

```sh
cargo +1.48 mutants
```

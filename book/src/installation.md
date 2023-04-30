# Installation

Install cargo-mutants from source:

```sh
cargo install --locked cargo-mutants
```

## Supported Rust versions

Building cargo-mutants requires a reasonably recent stable (or nightly or beta) Rust toolchain.

Currently cargo-mutants is
[tested with Rust 1.65](https://github.com/sourcefrog/cargo-mutants/actions/workflows/msrv.yml).

After installing cargo-mutants, you should be able to use it to run tests under
any toolchain, even toolchains that are far too old to build cargo-mutants, using the standard `+` option to `cargo`:

```sh
cargo +1.48 mutants
```

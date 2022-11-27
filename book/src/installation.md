# Installation

Install cargo-mutants from source:

```sh
cargo install --locked cargo-mutants
```

## Shell completion

The `--completions SHELL` emits completion scripts for the given shell.

The right place to install these depends on your shell and operating system.

For example, for Fish[^fishconf]:

```sh
cargo mutants --completions fish >~/.config/fish/conf.d/cargo-mutants-completions.fish
```

[^fishconf]: This command installs them to `conf.d` instead of `completions` because you may have completions for several `cargo` plugins.

## Supported Rust versions

Building cargo-mutants requires a reasonably recent stable (or nightly or beta) Rust toolchain.

Currently it is [tested with Rust 1.63](https://github.com/sourcefrog/cargo-mutants/actions/workflows/msrv.yml).

After installing cargo-mutants, you should be able to use it to run tests under
any toolchain, even toolchains that are far too old to build cargo-mutants, using the standard `+` option to `cargo`:

```sh
cargo +1.48 mutants
```

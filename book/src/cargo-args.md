# Passing options to Cargo

cargo-mutants runs `cargo test` to build and run tests. (With `--check`, it runs
`cargo check`.) Additional options can be passed in three different ways: to all
`cargo` commands; to `cargo test` only; and to the test binaries run by `cargo
test`.

There is not yet a way to pass options only to `cargo build` but not to `cargo test`.

## Feature flags

The `--features`, `--all-features`, and `--no-default-features` flags can be given to cargo-mutants and they will be passed down to cargo invocations.

For example, this can be useful if you have tests that are only enabled with a feature flag:

```shell
cargo mutants -- --features=fail/failpoints
```

## Arguments to all `cargo` commands

To pass more arguments to every Cargo invocation, use `--cargo-arg`, or the `additional_cargo_args` configuration key.
`--cargo-arg` can be repeated.

For example

```shell
cargo mutants -- --cargo-arg=--release
```

or in `.cargo/mutants.toml`:

```toml
additional_cargo_args = ["--all-features"]
```

## Arguments to `cargo test`

Command-line options following a `--` delimiter are passed through to
`cargo test` (or to [nextest](nextest.md), if you're using that).

For example, this can be used to pass `--all-targets` which (unobviously)
excludes doctests. (If the doctests are numerous and slow, and not relied upon to catch bugs, this can improve performance.)

```shell
cargo mutants -- --all-targets
```

These options can also be configured statically with the `additional_cargo_test_args` key in `.cargo/mutants.toml`:

```toml
additional_cargo_test_args = ["--jobs=1"]
```

## Arguments to test binaries

You can use a second double-dash to pass options through to the test targets:

```sh
cargo mutants -- -- --test-threads 1 --nocapture
```

(However, this may interact poorly with using `additional_cargo_test_args` in the configuration file,
as the argument lists are currently appended without specially handling the `--` separator.)

# Passing options to Cargo

Command-line options following a `--` delimiter are passed through to
`cargo test`, which can be used for example to exclude doctests (which tend to
be slow to build and run):

```sh
cargo mutants -- --all-targets
```

You can use a second double-dash to pass options through to the test targets:

```sh
cargo mutants -- -- --test-threads 1 --nocapture
```

These options can also be configured statically with the `additional_cargo_args` and `additional_cargo_test_args` keys in `.cargo/mutants.toml`.

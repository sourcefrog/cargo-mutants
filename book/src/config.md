# Config file

cargo-mutants looks for a `.cargo/mutants.toml` file in the root of the source
directory. If a config file exists, the values are appended to the corresponding
command-line arguments. (This may cause problems if you use `--` twice on the
command line to pass arguments to the inner test binary.)

Configured exclusions may be particularly when there are modules that are
inherently hard to test, and the project has made a decision to accept lower
test coverage for them.

Since Rust does not currently allow attributes such as `#[mutants::skip]` on `mod` statements or at module scope this is the only way to skip an entire module.

The following configuration options are supported:

```toml
exclude_globs = ["src/main.rs", "src/cache/*.rs"] # same as -e
examine_globs = ["src/important/*.rs"] # same as -f, test *only* these files

exclude_re = ["impl Debug"] # same as -E
examine_re = ["impl Serialize", "impl Deserialize"] # same as -F, test *only* matches

additional_cargo_args = ["--all-features"]
additional_cargo_test_args = ["--jobs=1"]
```

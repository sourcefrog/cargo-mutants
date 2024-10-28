# Workspace and package support

cargo-mutants supports testing Cargo workspaces that contain multiple packages.

The entire workspace tree is copied to the temporary directory (unless `--in-place` is used).

In workspaces with multiple packages, there are two considerations:

1. Which packages to generate mutants in, and
2. Which tests to run on those mutants.

## Selecting packages to mutate

By default, cargo-mutants selects packages to mutate using [similar heuristics to other Cargo commands](https://doc.rust-lang.org/cargo/reference/workspaces.html).

These rules work from the "starting directory", which is the directory selected by `--dir` or the current working directory.

* If `--workspace` is given, all packages in the workspace are mutated.
* If `--package` is given, the named packages are mutated.
* If the starting directory is in a package, that package is mutated. Concretely, this means: if the starting directory or its parents contain a `Cargo.toml` containing a `[package]` section.
* If the starting directory's parents contain a `Cargo.toml` with a `[workspace]` section but no `[package]` section, then the directory is said to be in a "virtual workspace". If the `[workspace]` section has a `default-members` key then these packages are mutated. Otherwise, all packages are mutated.

Selection of packages can be combined with [`--file`](skip_files.md) and other filters.

You can also use the `--file` options to restrict cargo-mutants to testing only files
from some subdirectory, e.g. with `-f "utils/**/*.rs"`. (Remember to quote globs
on the command line, so that the shell doesn't expand them.) You can use `--list` or
`--list-files` to preview the effect of filters.

## Selecting tests to run

For each baseline and mutant scenario, cargo-mutants selects some tests to see if the mutant is caught.
These selections turn into `--package` or `--workspace` arguments to `cargo test`.

By default, the baseline runs tests from all and only the packages for which mutants will be generated. That is, if the whole workspace is being tested, then it runs `cargo test --workspace`, and otherwise it selects all the packages.

By default, each mutant runs only the tests from the package that's being mutated.

If the `--test-workspace` arguments or `test_workspace` configuration key is set, then all tests from the workspace are run against each mutant.

If the `--test-package` argument or `test_package` configuration key is set then the specified packages are tested for all mutants.

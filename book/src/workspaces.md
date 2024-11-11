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

There are different behaviors for the baseline tests (before mutation), which run once for all packages, and then for the tests applied to each mutant.

These behaviors can be controlled by the `--test-workspace` and `--test-package` command line options and the corresponding configuration options.

By default, the baseline runs the tests from all and only the packages for which mutants will be generated. That is, if the whole workspace is being tested, then it runs `cargo test --workspace`, and otherwise runs tests for each selected package.

By default, each mutant runs only the tests from the package that's being mutated.

If the `--test-workspace=true` argument or `test_workspace` configuration key is set, then all tests from the workspace are run for the baseline and against each mutant.

If the `--test-package` argument or `test_package` configuration key is set then the specified packages are tested for the baseline and all mutants.

As for other options, the command line arguments have priority over the configuration file.

Like `--package`, the argument to `--test-package` can be a comma-separated list, or the option can be repeated.

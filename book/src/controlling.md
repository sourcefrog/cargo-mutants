# Controlling cargo-mutants

`cargo mutants` takes various options to control how it runs.

These options, can, in general, be passed on the command line, set in a `.cargo/mutants.toml`
file in the source tree, or passed in `CARGO_MUTANTS_` environment variables. Not every
method of setting an option is available for every option, however, as some would not
make sense or be useful.

For options that take a list of values, values from the configuration file are appended
to values from the command line.

For options that take a single value, the value from the command line takes precedence.

`--no-config` can be used to disable reading the configuration file.

## Execution order

By default, mutants are run in a randomized order, so as to surface results from
different parts of the codebase earlier. This can be disabled with
`--no-shuffle`, in which case mutants will run in order by file name and within each file in the order they appear in
the source.

## Source directory location

`-d`, `--dir`: Test the Rust tree in the given directory, rather than the source tree
enclosing the working directory where cargo-mutants is launched.

`--manifest-path`: Also selects the tree to test, but takes a path to a Cargo.toml file rather than a directory. (This is less convenient but compatible with other Cargo commands.)

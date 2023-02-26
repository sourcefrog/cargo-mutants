# Controlling cargo-mutants

`cargo mutants` takes various options to control how it runs. These options are shown in `cargo mutants --help` and are described in detail in this section.

By default, mutants are run in a randomized order, so as to surface results from
different parts of the codebase earlier. This can be disabled with
`--no-shuffle`, in which case mutants will run in the same order shown by
`--list`: in order by file name and within each file in the order they appear in
the source.

## Source directory location

`-d`, `--dir`: Test the Rust tree in the given directory, rather than the default directory.

## Console output

`-v`, `--caught`: Also print mutants that were caught by tests.

`-V`, `--unviable`: Also print mutants that failed `cargo build`.

`--no-times`: Don't print elapsed times.

## Environment variables

A few options that may be useful to set globally can be configured through environment 
variables:

* `CARGO_MUTANTS_JOBS`
* `CARGO_MUTANTS_TRACE_LEVEL`

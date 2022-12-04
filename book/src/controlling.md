# Controlling cargo-mutants

By default, mutants are run in a randomized order, so as to surface results from
different parts of the codebase earlier. This can be disabled with
`--no-shuffle`, in which case mutants will run in the same order shown by
`--list`: in order by file name and within each file in the order they appear in
the source.

`-d`, `--dir`: Test the Rust tree in the given directory, rather than the default directory.

`-v`, `--caught`: Also print mutants that were caught by tests.

`-V`, `--unviable`: Also print mutants that failed `cargo build`.

`--no-times`: Don't print elapsed times.

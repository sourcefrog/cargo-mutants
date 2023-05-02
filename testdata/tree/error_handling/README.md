# `error_handling` tree

This tree contains a function that can return an error.
The tests ignore the error case, so would fail to detect
a bug that causes an error to be returned.

(It's probably pretty likely that many Rust tests will `unwrap`
or `?` on the error and so implicitly catch it, but it's still
possible.)

With cargo-mutants `--error` option, we generate a mutant that
returns an error and so catch the missing coverage.

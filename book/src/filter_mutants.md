# Filtering functions and mutants

The `#[mutants::skip]` attributes let you permanently mark a function as skipped
in the source tree. You can also narrow down which functions or mutants are
tested just for a single run of `cargo mutants`.

Two options filter mutants by the full name of the mutant, which includes the
function name, file name, and a description of the change.

Mutant names are shown by `cargo mutants --list`, and the same command can be
used to preview the effect of filters.

`-F REGEX`, `--re REGEX`: Only test mutants whose full name matches the given regex.

`-E REGEX`, `--exclude-re REGEX`: Exclude mutants whose full name matches
the given regex.

These options may be repeated.

The regex matches a substring and can be anchored with `^` and `$`.

The regex syntax is defined by the [`regex`](https://docs.rs/regex/latest/regex/)
crate.

These filters are applied after filtering by filename, and `--re` is applied before
`--exclude-re`.

Examples:

- `-E 'impl Debug'` -- don't test `impl Debug` methods, because coverage of them
  might be considered unimportant.

- `-F 'impl Serialize' -F 'impl Deserialize'` -- test implementations of these
  two traits.

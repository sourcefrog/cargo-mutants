# Filtering functions and mutants

You can filter mutants by name, using the `--re` and `--exclude-re` command line
options and the corresponding `examine_re` and `exclude_re` config file options.

These options are useful if you want to run cargo-mutants just once, focusing on a subset of functions or mutants.

These options filter mutants by the full name of the mutant, which includes the
function name, file name, and a description of the change, as shown in the output of `cargo mutants --list`.

For example, one mutant name might be:

```text
src/outcome.rs:157: replace <impl Serialize for ScenarioOutcome>::serialize -> Result<S::Ok, S::Error> with Ok(Default::default())
```

Within this name, your regex can match any substring, including for example:

- The filename
- The trait, `impl Serialize`
- The struct name, `ScenarioOutcome`
- The function name, `serialize`
- The mutated return value, `with Ok(Default::default())`, or any part of it.

The regex matches a substring, but can be anchored with `^` and `$` to require that
it match the whole name.

The regex syntax is defined by the [`regex`](https://docs.rs/regex/latest/regex/)
crate.

These filters are applied after [filtering by filename](skip_files.md), and `--re` is applied before
`--exclude-re`.

Examples:

- `-E 'impl Debug'` -- don't test `impl Debug` methods, because coverage of them
  might be considered unimportant.

- `-F 'impl Serialize' -F 'impl Deserialize'` -- test implementations of these
  two traits.

## Configuring filters by name

Mutants can be filtered by name in the `.cargo/mutants.toml` file. The `exclude_re` and `examine_re` keys are each a list of strings.

This can be helpful
if you want to systematically skip testing implementations of certain traits, or functions
with certain names.

From cargo-mutants 23.11.2 onwards, if the command line options are given then the corresponding config file option is ignored.

For example:

```toml
exclude_re = ["impl Debug"] # same as -E
```

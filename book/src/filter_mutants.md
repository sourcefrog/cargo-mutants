# Filtering functions and mutants

You can also filter mutants by name, using the `--re` and `--exclude-re` command line
options and the corresponding `examine_re` and `exclude_re` config file options.

These options are useful if you want to run cargo-mutants just once, focusing on a subset of functions or mutants.

These options filter mutants by the full name of the mutant, which includes the
function name, file name, and a description of the change, as shown in list.

For example, one mutant name might be:

```text
src/outcome.rs:157: replace <impl Serialize for ScenarioOutcome>::serialize -> Result<S::Ok, S::Error> with Ok(Default::default())
```

Within this name, your regex can match any substring, including for example:

- The filename
- The trait, `impl Serialize`
- The struct name, `ScenarioOutcome`
- The function name, `serialize`
- The mutated return value, `with Ok(Defualt::default())`, or any part of it.

Mutants can also be filtered by name in the `.cargo/mutants.toml` file, for example:

Regexes from the config file are appended to regexes from the command line.

The regex matches a substring, but can be anchored with `^` and `$` to require that
it match the whole name.

The regex syntax is defined by the [`regex`](https://docs.rs/regex/latest/regex/)
crate.

These filters are applied after filtering by filename, and `--re` is applied before
`--exclude-re`.

Examples:

- `-E 'impl Debug'` -- don't test `impl Debug` methods, because coverage of them
  might be considered unimportant.

- `-F 'impl Serialize' -F 'impl Deserialize'` -- test implementations of these
  two traits.

Or in `.cargo/mutants.toml`:

```toml
exclude_re = ["impl Debug"] # same as -E
examine_re = ["impl Serialize", "impl Deserialize"] # same as -F, test *only* matches
```

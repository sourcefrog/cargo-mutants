# cargo-mutants changelog

## unreleased

* Skip functions or modules marked `#[test]`, `#[cfg(test)]` or
  `#[mutants::skip]`.

* Early steps towards type-guided mutations: generate mutations of `true`
  and `false` for functions that return `bool`, and empty and arbitrary strings
  for functions returning `String`.

* Rename `--list-mutants` to just `--list`.

* New `--list --json`.

* Very basic mutation of `Result<_, _>` to return `Ok(Default::default())`.

## 0.0.2

* Functions that should not be mutated can be marked with `#[mutants::skip]`
  from the [`mutants`](https://crates.io/crates/mutants) helper crate.

## 0.0.1
 
First release.

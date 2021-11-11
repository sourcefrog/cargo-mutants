# cargo-mutants changelog

## unreleased

  * Fixed `cargo install cargo-mutants` (sometimes?) failing due to the
    `derive` feature not getting set on the `serde` dependency.

  * Show progress while copying the tree.

  * Respect the `$CARGO` environment variable so that the same toolchain is
    used to run tests as was used to invoke `cargo mutants`. Concretely, `cargo
    +nightly mutants` should work correctly.

## 0.0.3

Released 2021-11-06

  * Skip functions or modules marked `#[test]`, `#[cfg(test)]` or
    `#[mutants::skip]`.

  * Early steps towards type-guided mutations: 

    * Generate mutations of `true` and `false` for functions that return `bool`
    * Empty and arbitrary strings for functions returning `String`.
    * Return `Ok(Default::default())` for functions that return `Result<_, _>`.

  * Rename `--list-mutants` to just `--list`.

  * New `--list --json`.

  * Colored output makes test names and mutations easier to read (for me at least.)

  * Return distinct exit codes for different situations including that uncaught
    mutations were found.

## 0.0.2

  * Functions that should not be mutated can be marked with `#[mutants::skip]`
    from the [`mutants`](https://crates.io/crates/mutants) helper crate.

## 0.0.1
 
First release.

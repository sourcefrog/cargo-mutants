# Contributing to cargo-mutants

Pull requests are welcome. If the change is not obvious please feel free to open
a bug or Github discussion first.

## Code of Conduct

This project is conducted in accord with the [Rust Code of
Conduct](https://www.rust-lang.org/policies/code-of-conduct).

## UI Style

- Always print paths with forward slashes, even on Windows. Use `path_slash`.

## Rust Style

Generally, variables and parameters should be the snake_case version of their
type name, unless that would be unclear: `source_tree: SourceTree`.

For this relatively small project I'm moving towards `pub use` of all public
names into `main.rs`, so that other implementation modules can just
`use crate::*`.

Try to keep one major class or separation of concern per mod, with
implementation details being private. However, fields that would have trivial
getters and that don't break the abstraction can be public.

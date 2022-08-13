# Contributing to cargo-mutants

Pull requests are welcome.

If the change is not obvious please feel free to open a bug or Github discussion first.

If you are interested in working on a bug or feature please say so on the bug first to avoid wasted work.

## Code of Conduct

This project is conducted in accord with the [Rust Code of
Conduct](https://www.rust-lang.org/policies/code-of-conduct).

## UI Style

- Always print paths with forward slashes, even on Windows. Use `path_slash`.

## Rust Style

Generally, variables and parameters should be the `snake_case` version of their
type name: `source_tree: SourceTree`. However if that would be unclear or ambiguous, use a different name that does not repeat the type: `src: &Path, dest: &Path`.

For this relatively small project I'm moving towards `pub use` of all public
names into `main.rs`, so that other implementation modules can just
`use crate::*`.

Try to keep one major class or separation of concern per mod, with
implementation details being private. However, fields that would have trivial
getters and that don't break the abstraction can be public, since this crate does not provide a library API. Public-ness is used mostly as a marker that something is an implementation detail of a module.

Please run `cargo fmt` and `cargo clippy`. These are checked in CI.

## Testing

Of course, please add tests for new features or bug fixes.

Cargo-mutants is primarily tested on its public interface, which is the command line. These tests live in `tests/cli` and generally have the form of:

1. Make a copy of a `testdata` tree, so that it's not accidentally mutated.
2. Run a `cargo-mutants` command on it.
3. Inspect the stdout, return code, or `mutants.out`.

`cargo-mutants` runs as a subprocess of the test process so that we get the most realistic view of its behavior. In some cases it is run via the `cargo` command to test that this level of indirection works properly.

### `testdata` trees

The primary means of testing is Rust source trees under `testdata/tree`: you can copy an existing tree and modify it to show the new behavior that you want to test.

There is a general test that runs `cargo mutants --list` on each tree.

A selection of test trees are available for testing different scenarios. If there is an existing suitable tree, please use it. If you need to test a situation that is not covered yet, please add a new tree.

Please describe the purpose of the testdata tree in a `README.md` within the tree.

To make a new tree you can copy an existing tree, but make sure to change the package name in its `Cargo.toml`.

All the trees need to be mentioned in the top-level `Cargo.toml` as either included in the workspace or excluded from it. Trees that build and test successfully and that are not otherwise incompatible should be included.

### Insta snapshots

Many tests use Insta <https://insta.rs> to assert the expected output. Insta makes it easy to review and accept changes to expected output, either when there's a real functional change or when for example line numbers or output formatting changes.

To conveniently review changed output, `cargo install cargo-insta` and then run `cargo insta test --review` etc.

### Test performance

CLI tests spawn a new process which can be slightly slow, especially on Windows, and especially if they actually test the mutants.

So, try to keep `testdata` trees reasonably minimal for whatever they're intended to test.

### Unit tests

Although we primarily want to test the public interface (which is the command line), unit tests can be added in a `mod test {}` within the source tree for any behavior that is inconvenient to exercise from the command line.

### Debugging code under test

When the tests run, `cargo test` runs a test suite binary from `target`, which then runs `cargo-mutants` as a subprocess. (Or, in some cases, it runs `cargo` which runs `cargo-mutants`.)

As a result, attaching a debugger to the test binary will let you see the code that launches the subprocess and that inspects the output, but it won't let you step through cargo-mutants itself, which is probably the most interesting part.

Probably the easiest path is to just make note of the command run by the test, and then run that command yourself, under a debugger, outside of the test suite. For example, `./target/debug/cargo-mutants -d ./testdata/tree/factorial --list`.

You may wish to turn off the timeouts with `-t 0`.

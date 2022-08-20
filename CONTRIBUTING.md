# Contributing to cargo-mutants

Pull requests are welcome.

If the change is not obvious please feel free to open a bug or Github discussion first.

If you are interested in working on a bug or feature please say so on the bug first to avoid wasted work, and feel free to talk or ask about the approach.

Please also read the [DESIGN.md](DESIGN.md) file for technical information not specifically about posting contributions.

## Code of Conduct

This project is conducted in accord with the [Rust Code of
Conduct](https://www.rust-lang.org/policies/code-of-conduct).

## Rust Style

Generally, variables and parameters should be the `snake_case` version of their
type name: `source_tree: SourceTree`. However if that would be unclear or ambiguous, use a different name that does not repeat the type: `src: &Path, dest: &Path`.

Try to keep one major class or separation of concern per mod, with
implementation details being private. However, fields that would have trivial
getters and that don't break the abstraction can be `public`, since this crate does not provide a library API. `public` is used mostly as a marker that something is an implementation detail of a module.

Please run `cargo fmt` and `cargo clippy`. These are checked in CI.

## Testing

Of course, please add tests for new features or bug fixes, and see the _Testing_ section of [the design doc](DESIGN.md).

### Insta snapshots

Many tests use [Insta](https://insta.rs) to assert the expected output. Insta makes it easy to review and accept changes to expected output, either when there's a real functional change or when for example line numbers or output formatting changes.

To conveniently review changed output, `cargo install cargo-insta` and then run `cargo insta test --review` etc.

### Test performance

CLI tests spawn a new process which can be slightly slow, especially on Windows, and especially if they actually test the mutants.

Try to keep `testdata` trees reasonably minimal for whatever they're intended to test.

### Debugging code under test

When the tests run, `cargo test` runs a test suite binary from `target`, which then runs `cargo-mutants` as a subprocess. (Or, in some cases, it runs `cargo` which runs `cargo-mutants`.)

As a result, attaching a debugger to the test binary will let you see the code that launches the subprocess and that inspects the output, but it won't let you step through cargo-mutants itself, which is probably the most interesting part.

Probably the easiest path is to just make note of the command run by the test, and then run that command yourself, under a debugger, outside of the test suite. For example, `./target/debug/cargo-mutants -d ./testdata/tree/factorial --list`.

You may wish to turn off the timeouts with `-t 0`.

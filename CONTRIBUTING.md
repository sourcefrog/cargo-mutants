# Contributing to cargo-mutants

If you're interested in adding a feature or fixing a bug, thank you! Please
start by reading this document and opening a Github discussion or bug about the
thing you want to do, to avoid wasted work. void wasted work, and feel free to
talk or ask about the approach.

Please also read the [DESIGN.md](DESIGN.md) file for technical information not specifically about posting contributions.

## Code of Conduct

This project is conducted in accord with the [Rust Code of
Conduct](https://www.rust-lang.org/policies/code-of-conduct).

## Try it on a new tree

One of the most helpful things you can do is to try cargo-mutants on a new tree: either your own project
or an important open source project:

* Did cargo-mutants hang, error, or otherwise fail to run? If so a bug with reproduction instructions would be very helpful.
* Were the mutants interesting or helpful in understanding coverage or test quality? Did it generate any new tests that were accepted into the tree?

## Rust Style

Generally, variables and parameters should be the `snake_case` version of their
type name: `source_tree: SourceTree`. However if that would be unclear or ambiguous, use a different name that does not repeat the type: `src: &Path, dest: &Path`.

Try to keep one major class or separation of concern per mod, with
implementation details being private. However, fields that would have trivial
getters and that don't break the abstraction can be `public`, since this crate does not provide a library API. `public` is used mostly as a marker that something is an implementation detail of a module.

Please run `cargo fmt` and `cargo clippy`. These are checked in CI.

## Testing

Of course, please add tests for new features or bug fixes. See also the _Testing_ section of [the design doc](DESIGN.md).

### Running the tests

cargo-mutants tests require [`cargo-nextest`](https://nexte.st/) to be installed, so that they can exercise `--test-tool=nextest`.

cargo-mutants tests can be run under either `cargo test` or `cargo nextest run`.

### Test naming

Tests should have names that read like English sentences (or subsentences)
asserting a fact about how the program behaves, like
`copy_testdata_doesnt_include_build_artifacts`. It's fine if this makes the
test function names relatively long.

However, also try to avoid "noise" or low-value words in test names.
`show_version` is clear enough and does not need to be
`when_run_with_show_version_it_prints_the_version_to_stdout`.

As with other code, if you feel you need to add a comment to explain what it
does, then first consider whether the test can have a better name.

If the test exercises a particular test tree, option, or function, make sure
that name literally occurs within the test name.

### Insta snapshots

Many tests use [Insta](https://insta.rs) to assert the expected output. Insta makes it easy to review and accept changes to expected output, either when there's a real functional change or when for example line numbers or output formatting changes.

To conveniently review changed output, `cargo install cargo-insta` and then run `cargo insta test --review` etc.

### Test performance

CLI tests spawn a new process which can be slightly slow, especially on Windows, and especially if they actually test the mutants.

Try to keep `testdata` trees reasonably minimal for whatever they're intended to test.

### Debugging code under test

When the tests run, `cargo test` runs a test suite binary from `target`, which then runs `cargo-mutants` as a subprocess. (Or, in some cases, it runs `cargo` which runs `cargo-mutants`.)

As a result, attaching a debugger to the test binary will let you see the code that launches the subprocess and that inspects the output, but it won't let you step through cargo-mutants itself, which is probably the most interesting part.

Probably the easiest path is to just make note of the command run by the test, and then run that command yourself, under a debugger, outside of the test suite. For example, `./target/debug/cargo-mutants -d ./testdata/factorial --list`.

You may wish to turn off the timeouts with `-t 0`.

## Generating new mutations

The largest area for new work at the moment is in generating new mutations. Most of the code for this is in `visit.rs`.

If you look in `mutants.out/debug.log` you can see messages like `Return type is not recognized, trying Default`.
These might be good places to add a new more specific pattern.

Also `mutants.out/unviable.txt` might suggest ways to generate new patterns that are viable.

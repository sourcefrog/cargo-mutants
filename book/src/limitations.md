# Limitations, caveats, known bugs, and future enhancements

## Cases where cargo-mutants _can't_ help

cargo-mutants can only help if the test suite is hermetic: if the tests are
flaky or non-deterministic, or depend on external state, it will draw the wrong
conclusions about whether the tests caught a bug.

If you rely on testing the program's behavior by manual testing, or by an
integration test not run by `cargo test`, then cargo-mutants can't know this,
and will only tell you about gaps in the in-tree tests. It may still be helpful
to run mutation tests on only some selected modules that do have in-tree tests.

Running cargo-mutants on your code won't, by itself, make your code better. It
only helps suggest places you might want to improve your tests, and that might
indirectly find bugs, or prevent future bugs. Sometimes the results will point
out real current bugs. But it's on you to follow up. (However, it's really easy
to run, so  you might as well look!)

cargo-mutants typically can't do much to help with crates that primarily
generate code using macros or build scripts, because it can't "see" the code
that's generated. (You can still run it, but it's may generate very few
mutants.)

## Stability

cargo-mutants behavior, output formats, command-line syntax, json output
formats, etc, may change from one release to the next.

## Limitations and known bugs

cargo-mutants currently only supports mutation testing of Rust code that builds
using `cargo` and where the tests are run using `cargo test`. Support for other tools such as Bazel or Nextest could in principle be added.

cargo-mutants sees the AST of the tree but doesn't fully "understand" the types, so sometimes generates unviable mutants or misses some opportunities to generate interesting mutants.

cargo-mutants reads `CARGO_ENCODED_RUSTFLAGS` and `RUSTFLAGS` environment variables, and sets `CARGO_ENCODED_RUSTFLAGS`.  It does not read `.cargo/config.toml` files, and so any rust flags set there will be ignored.

cargo-mutants does not yet understand conditional compilation, such as
`#[cfg(target_os = "linux")]`. It will report functions for other platforms as
missed, when it should know to skip them.

### Support for other build tools

cargo-mutants currently only works with Cargo, but could in principle be extended to work with other build tools such as Bazel.

cargo-mutants contains two main categories of code, which are mostly independent:

1. Code for reading Rust source code, parsing it, and mutating it: this is not
   specific to Cargo.

2. Code for finding the modules to mutate and their source files, finding the tree to copy, adjusting paths after it is copied, and finally running builds and tests. This is very Cargo-specific, but should not be too hard to generalize.

The main precondition for supporting Bazel is a realistic test case: preferably an open source Rust tree built with Bazel, or at least a contributor with a Bazel-based Rust tree who is willing to help test and debug and to produce some test cases.

(See <https://github.com/sourcefrog/cargo-mutants/issues/77> for more discussion.)

## Caution on side effects

cargo-mutants builds and runs code with machine-generated modifications. This is
generally fine, but if the code under test  has side effects such as writing or
deleting files, running it with mutations might conceivably have unexpected
effects, such as deleting the wrong files, in the same way that a bug might.

If you're concerned about this, run cargo-mutants in a container or virtual
machine.

cargo-mutants never modifies the original source tree, other than writing a
`mutants.out` directory, and that can be sent elsewhere with the `--output`
option. All mutations are applied and tested in a copy of the source tree.

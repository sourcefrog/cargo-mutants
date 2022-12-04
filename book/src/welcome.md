# Welcome to cargo-mutants

cargo-mutants is a mutation testing tool for Rust. It helps you improve your
program's quality by finding functions whose body could be replaced without
causing any tests to fail. Each such case indicates, perhaps, a gap in semantic
code coverage by your tests, where a bug might be lurking.

**The goal of cargo-mutants is to be _easy_ to run on any Rust source tree, and
to tell you something _interesting_ about areas where bugs might be lurking or
the tests might be insufficient.** ([More about these goals](goals.md).)

## How is mutation testing different to coverage measurement?

Coverage measurements tell you which lines of code (or other units) are reached
while running a test. They don't tell you whether the test really _checks_
anything about the behavior of the code.

For example, a function that writes a file and returns a `Result` might be
covered by a test that checks the return value, but not by a test that checks
that the file was actually written. cargo-mutants will try mutating the function
to simply return `Ok(())` and report that this was not caught by any tests.

Historically, rust coverage measurements have required manual setup of several
OS and toolchain-dependent tools, although this is improving. `cargo-mutants` is
simple to install and run on any Rust source tree and requires no special
toolchain integrations, and no special tools to interpret the results.

Coverage tools also in some cases produce output that is hard to interpret, with
lines sometimes shown as covered or not due to toolchain quirks that aren't easy
to map to direct changes to the test suite. cargo-mutants produces a direct list
of changes that are not caught by the test suite, which can be quickly reviewed
and prioritized.

One drawback of mutation testing is that it runs the whole test suite once per
generated mutant, so it can be slow on large trees. There are [some techniques
to speed up cargo-mutants](performance.md), including [running multiple tests in
parallel](parallel.md).

## How is mutation testing different to fuzzing?

Fuzzing is a technique for finding bugs by feeding pseudo-random inputs to a
program, and is particularly useful on programs that parse complex or untrusted
inputs such as binary file formats or network protocols.

Mutation testing makes algorithmically-generated changes to a copy of the
program source, and measures whether the test suite catches the change.

The two techniques are complementary. Although some bugs might be found by
either technique, fuzzing will tend to find bugs that are triggered by complex
or unusual inputs, whereas mutation testing will tend to point out logic that
might be correct but that's not tested.

## Cases where cargo-mutants _can't_ help

cargo-mutants currently only supports mutation testing of Rust code that builds
using `cargo` and where the tests are run using `cargo test`.

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

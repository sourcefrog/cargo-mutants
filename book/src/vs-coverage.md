# How is mutation testing different to coverage measurement?

Coverage measurements tell you which lines of code (or other units) are reached
while running a test. They don't tell you whether the test really _checks_
anything about the behavior of the code.

For example, a function that writes a file and returns a `Result` might be
covered by a test that checks the return value, but not by a test that checks
that the file was actually written. cargo-mutants will try mutating the function
to simply return `Ok(())` and report that this was not caught by any tests.

Historically, rust coverage measurements have required manual setup of several
OS and toolchain-dependent tools, although this is improving. Because
`cargo-mutants` just runs `cargo` it has no OS-specific or tight toolchain
integrations, and so is simple to install and run on any Rust source tree.
cargo-mutants also needs no special tools to view or interpret the results.

Coverage tools also in some cases produce output that is hard to interpret, with
lines sometimes shown as covered or not due to toolchain quirks that aren't easy
to map to direct changes to the test suite. cargo-mutants produces a direct list
of changes that are not caught by the test suite, which can be quickly reviewed
and prioritized.

One drawback of mutation testing is that it runs the whole test suite once per
generated mutant, so it can be slow on large trees with slow test suites. There
are [some techniques to speed up cargo-mutants](performance.md), including
[running multiple tests in parallel](parallel.md).

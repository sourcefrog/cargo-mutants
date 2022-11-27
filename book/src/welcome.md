# Welcome to cargo-mutants

cargo-mutants is a mutation testing tool for Rust. It helps you improve your
program's quality by finding functions whose body could be replaced without
causing any tests to fail.

Coverage measurements can be helpful, but they really tell you what code is
_reached_ by a test, and not whether the test really _checks_ anything about the
behavior of the code. Mutation tests give different information, about whether
the tests really check the code's behavior.

TODO: Some motivating examples. How does mutation testing help?

TODO: How is this different to coverage, etc?

## Goals of cargo-mutants

TODO

# cargo-mutants

<https://github.com/sourcefrog/cargo-mutants>

[![Tests](https://github.com/sourcefrog/cargo-mutants/actions/workflows/tests.yml/badge.svg?branch=main&event=push)](https://github.com/sourcefrog/cargo-mutants/actions/workflows/tests.yml?query=branch%3Amain)
[![crates.io](https://img.shields.io/crates/v/cargo-mutants.svg)](https://crates.io/crates/cargo-mutants)
[![libs.rs](https://img.shields.io/badge/libs.rs-cargo--mutants-blue)](https://lib.rs/crates/cargo-mutants)

cargo-mutants is a mutation testing tool for Rust. It helps you improve your
program's quality by finding functions whose body could be replaced without
causing any tests to fail.

Coverage measurements can be helpful, but they really tell you what code is
_reached_ by a test, and not whether the test really _checks_ anything about the
behavior of the code. Mutation tests give different information, about whether
the tests really check the code's behavior.

The goal of cargo-mutants is to be _easy_ to run on any Rust source tree, and
to tell you something _interesting_ about areas where bugs might be lurking or
the tests might be insufficient.

**The main documentation is the user guide at <https://mutants.rs/>.**

## Install

```sh
cargo install --locked cargo-mutants
```

## Quick start

From within a Rust source directory, just run

```sh
cargo mutants
```

## Further reading

**The main documentation is the user guide at <https://mutants.rs/>.**

See also:

- [How cargo-mutants compares to other techniques and tools](https://github.com/sourcefrog/cargo-mutants/wiki/Compared).
- [Design notes](DESIGN.md)
- [Contributing](CONTRIBUTING.md)
- [Release notes](NEWS.md)

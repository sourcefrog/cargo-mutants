# cargo-mutants

<https://github.com/sourcefrog/cargo-mutants>

[![Tests](https://github.com/sourcefrog/cargo-mutants/actions/workflows/tests.yml/badge.svg?branch=main&event=push)](https://github.com/sourcefrog/cargo-mutants/actions/workflows/tests.yml?query=branch%3Amain)
[![crates.io](https://img.shields.io/crates/v/cargo-mutants.svg)](https://crates.io/crates/cargo-mutants)
[![libs.rs](https://img.shields.io/badge/libs.rs-cargo--mutants-blue)](https://lib.rs/crates/cargo-mutants)
[![GitHub Sponsors](https://img.shields.io/badge/Sponsor-%E2%9D%A4-%23db61a2.svg?&logo=github&logoColor=white&labelColor=181717&style=flat-square)](https://github.com/sponsors/sourcefrog)

cargo-mutants helps you improve your
program's quality by finding places where bugs could be inserted without
causing any tests to fail.

Coverage measurements can be helpful, but they really tell you what code is
_reached_ by a test, and not whether the test really _checks_ anything about the
behavior of the code. Mutation tests give different information, about whether
the tests really check the code's behavior.

The goal of cargo-mutants is to be _easy_ to run on any Rust source tree, and
to tell you something _interesting_ about areas where bugs might be lurking or
the tests might be insufficient.

**The main documentation is the user guide at <https://mutants.rs/>.**

## Prerequisites

cargo-mutants can help on trees with non-flaky tests that run under `cargo test` or [`cargo nextest run`](https://nexte.st/).

## Install

```sh
cargo install --locked cargo-mutants
```

You can also install using [cargo-binstall](https://github.com/cargo-bins/cargo-binstall) or from binaries attached to GitHub releases.

## Quick start

From within a Rust source directory, just run

```sh
cargo mutants
```

To generate mutants in only one file:

```sh
cargo mutants -f src/something.rs
```

## Help advance cargo-mutants

If you use cargo-mutants or just like the idea you can help it get better:

* [Post an experience report in GitHub discussions](https://github.com/sourcefrog/cargo-mutants/discussions), saying whether it worked, failed, found interesting results, etc.
* [Sponsor development](https://github.com/sponsors/sourcefrog)

## Project status

As of January 2024 this is an actively-maintained spare time project. I expect to make [releases](https://github.com/sourcefrog/cargo-mutants/releases) about every one or two months.

It's very usable at it is and there's room for lots more future improvement,
especially in adding new types of mutation.

This software is provided as-is with no warranty of any kind.

## Further reading

See also:

* [cargo-mutants manual](https://mutants.rs/)
* [How cargo-mutants compares to other techniques and tools](https://github.com/sourcefrog/cargo-mutants/wiki/Compared).
* [Design notes](DESIGN.md)
* [Contributing](CONTRIBUTING.md)
* [Release notes](NEWS.md)
* [Discussions](https://github.com/sourcefrog/cargo-mutants/discussions)

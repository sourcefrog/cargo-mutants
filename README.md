# cargo-mutants

<https://github.com/sourcefrog/cargo-mutants>

[![Tests](https://github.com/sourcefrog/cargo-mutants/actions/workflows/tests.yml/badge.svg?branch=main&event=push)](https://github.com/sourcefrog/cargo-mutants/actions/workflows/tests.yml?query=branch%3Amain)
[![crates.io](https://img.shields.io/crates/v/cargo-mutants.svg)](https://crates.io/crates/cargo-mutants)
[![libs.rs](https://img.shields.io/badge/libs.rs-cargo--mutants-blue)](https://lib.rs/crates/cargo-mutants)
[![GitHub Sponsors](https://img.shields.io/badge/Sponsor-%E2%9D%A4-%23db61a2.svg?&logo=github&logoColor=white&labelColor=181717&style=flat-square)](https://github.com/sponsors/sourcefrog)
[![Donate](https://img.shields.io/badge/Stripe-Donate-blue)](https://donate.stripe.com/fZu6oH6ry9I6epVfF7dfG00)

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

**For more background, see the [slides](https://docs.google.com/presentation/d/1YDwHz6ysRRNYRDtv80EMRAs4FQu2KKQ-IbGu2jrqswY/edit?pli=1&slide=id.g2876539b71f_0_0) and [video](https://www.youtube.com/watch?v=PjDHe-PkOy8&pp=ygUNY2FyZ28tbXV0YW50cw%3D%3D) from my Rustconf 2024 talk.**

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

## Docker

The cargo-mutants Docker image is just the official Rust Docker image with cargo-mutants and cargo-nextest preinstalled.

From within a Rust source directory, just run

```sh
docker run --rm -v .:/app ghcr.io/sourcefrog/cargo-mutants:latest
```

Available Docker tags are

* `<major>.<minor>.<patch>`, `<major>.<minor>`, `<major>`: Tracks a specific cargo-mutants release.
 `latest`: Tracks the latest cargo-mutants release.
* `edge`: Tracks the main branch in the cargo-mutants repository.

A suffix can be added to each tag to specify which Rust Docker image the cargo-mutants Docker image is based on.

* No suffix: Built on the `rust:latest` Docker image. Eg. `0.27.1`, `latest`, `edge`.
* `-slim` suffix: Built on the `rust:slim` Docker image. Eg. `0.27.1-slim`, `latest-slim`, `edge-slim`.
* `-alpine` suffix: Built on the `rust:alpine` Docker image. Eg. `0.27.1-alpine`, `latest-alpine`, `edge-alpine`.

## Integration with CI

The [manual includes instructions and examples for automatically testing mutants in CI](https://mutants.rs/ci.html), including incremental testing of pull requests and full testing of the development branch.

## Help advance cargo-mutants

If you use cargo-mutants or just like the idea you can help it get better:

* [Post an experience report in GitHub discussions](https://github.com/sourcefrog/cargo-mutants/discussions), saying whether it worked, failed, found interesting results, etc.
* [Sponsor development](https://github.com/sponsors/sourcefrog)

## Project status

As of August 2026 this is an semi-actively-maintained spare time project, although I've recently had less time to work on it. PRs and bugs will still be read with higher latency. I expect to make [releases](https://github.com/sourcefrog/cargo-mutants/releases) every few months at worst.

It's very usable as it is and there's room for lots more future improvement, especially in adding new types of mutation.

If you try it out on your project, [I'd love to hear back in a github discussion](https://github.com/sourcefrog/cargo-mutants/discussions/categories/general) whether it worked well or what could be better:

* Did it work on your tree? Did you need to set any options or do any debugging to get it working?
* Did it find meaningful gaps in testing? Where there too many false positives?
* What do you think would make it better or easier?

This software is provided as-is with no warranty of any kind.

## Further reading

See also:

* [cargo-mutants manual](https://mutants.rs/)
* [How cargo-mutants compares to other techniques and tools](https://github.com/sourcefrog/cargo-mutants/wiki/Compared).
* [Design notes](DESIGN.md)
* [Contributing](CONTRIBUTING.md)
* [Release notes](NEWS.md)
* [Discussions](https://github.com/sourcefrog/cargo-mutants/discussions)

# cargo-mutants

<https://github.com/sourcefrog/cargo-mutants>

[![Tests](https://github.com/sourcefrog/cargo-mutants/actions/workflows/tests.yml/badge.svg?branch=main&event=push)](https://github.com/sourcefrog/cargo-mutants/actions/workflows/tests.yml?query=branch%3Amain)
[![crates.io](https://img.shields.io/crates/v/cargo-mutants.svg)](https://crates.io/crates/cargo-mutants)
[![libs.rs](https://img.shields.io/badge/libs.rs-cargo--mutants-blue)](https://lib.rs/crates/cargo-mutants)
![Maturity: Beta](https://img.shields.io/badge/maturity-beta-blue.svg)

cargo-mutants is a mutation testing tool for Rust. It helps you improve your
program's quality by finding functions whose body could be replaced without
causing any tests to fail.

Coverage measurements can be helpful, but they really tell you what code is
_reached_ by a test, and not whether the test really _checks_ anything about the
behavior of the code. Mutation tests give different information, about whether
the tests really check the code's behavior.

**CAUTION**: This tool builds and runs code with machine-generated
modifications. If the code under test, or the test suite, has side effects such
as writing or deleting files, running it with mutations may be dangerous. Think
first about what side effects the test suite could possibly have, and/or run it
in a restricted or disposable environment.

## Install

```sh
cargo install cargo-mutants
```

## Using cargo-mutants

Just run `cargo mutants` in a Rust source directory, and it will point out
functions that may be inadequately tested:

```sh
; cargo mutants
Freshen source tree ... ok in 0.031s
Copy source and build products to scratch directory ... 192 MB in 0.116s
Unmutated baseline ... ok in 0.235s
Auto-set test timeout to 20.0s
Found 17 mutants to test
src/lib.rs:168: replace <??>::new -> CopyOptions < 'f > with Default::default() ... NOT CAUGHT in 0.736s
src/lib.rs:386: replace Error::source -> Option < & (dyn std :: error :: Error + 'static) > with Default::default() ... NOT CAUGHT in 0.643s
src/lib.rs:485: replace copy_symlink -> Result < () > with Ok(Default::default()) ... NOT CAUGHT in 0.767s
```

In v0.5.1 of the `cp_r` crate, the `copy_symlink` function was reached by a test
but not adequately tested.

### Command-line options

`-d`, `--dir`: Test the Rust tree in the given directory, rather than the default directory.

`-f`, `--file FILE`: Mutate only functions in files matching the given name or
glob. If the glob contains `/` it matches against the path from the source tree
root; otherwise it matches only against the file name.

`--list`: Show what mutants could be generated, without running them.

`--diff`: With `--list`, also include a diff of the source change for each mutant.

`--json`: With `--list`, show the list in json.

`--check`: Run `cargo check` on all generated mutants to find out which ones are viable, but don't actually run the tests.

`--no-copy-target`: Don't copy the `/target` directory from the source, and
don't freshen the source directory before copying it. The first "baseline" build
in the scratch directory will be a clean build with nothing in `/target`. This
will typically be slower (which is why `/target` is copied by default) but it
might help in debugging any issues with the build. (And, in niche cases where
there is a very large volume of old unreferenced content in `/target`, it might
conceivably be faster, but that's probably better dealt with by `cargo clean` in
the source directory.)

`--no-shuffle`: Test mutants in the fixed order they're found in the source
rather than the default behavior of running them in random order. (Shuffling is
intended to surface new and different mutants earlier on repeated partial runs
of cargo-mutants.)

`-v`, `--caught`: Also print mutants that were caught by tests.

`-V`, `--unviable`: Also print mutants that failed `cargo build`.

`--no-times`: Don't print elapsed times.

`--timeout`: Set a fixed timeout for each `cargo test` run, to catch mutations
that cause a hang. By default a timeout is automatically determined.

`--cargo-arg`: Passes the option argument to `cargo check`, `build`, and `test`.
For example, `--cargo-arg --release`.

### Passing arguments to `cargo test`

Command-line options following a `--` delimiter are passed through to
`cargo test`, which can be used for example to exclude doctests (which tend to
be slow to build and run):

```sh
cargo mutants -- --all-targets
```

You can use a second double-dash to pass options through to the test targets:

```sh
cargo mutants -- -- --test-threads 1 --nocapture
```

### Understanding the results

If tests fail in a clean copy of the tree, there might be an (intermittent)
failure in the source directory, or there might be some problem that stops them
passing when run from a different location, such as a relative `path` in
`Cargo.toml`. Fix this first.

Otherwise, cargo mutants generates every mutant it can. All mutants fall in to
one of these categories:

- **caught** — A test failed with this mutant applied. This is a good sign about
  test coverage. You can look in `mutants.out/log` to see which tests failed.

- **missed** — No test failed with this mutation applied, which seems to
  indicate a gap in test coverage. Or, it may be that the mutant is
  undistinguishable from the correct code. You may wish to add a better test, or
  mark that the function should be skipped.

- **unviable** — The attempted mutation doesn't compile. This is inconclusive about test coverage and
  no action is needed, but indicates an opportunity for cargo-mutants to either
  generate better mutants, or at least not generate unviable mutants.

- **timeout** — The mutation caused the test suite to run for a long time, until it was eventually killed. You might want to investigate the cause and potentially mark the function to be skipped.

By default only missed mutants and timeouts are printed because they're the most actionable. Others can be shown with the `-v` and `-V` options.

### Skipping functions

To mark functions so they are not mutated:

1. Add a Cargo dependency on the [mutants](https://crates.io/crates/mutants)
   crate, version "0.0.3" or later. (This must be a regular `dependency` not a
   `dev-dependency`, because the annotation will be on non-test code.)

2. Mark functions with `#[mutants::skip]` or other attributes containing `mutants::skip` (e.g. `#[cfg_attr(test, mutants::skip)`).

See `testdata/tree/hang_avoided_by_attr/` for an example.

The crate is tiny and the attribute has no effect on the compiled code. It only
flags the function for cargo-mutants.

**Note:** Currently, `cargo-mutants` does not (yet) evaluate attributes like `cfg_attr`, it only looks for the sequence `mutants::skip` in the attribute.

### Exit codes

- **0**: Success. No mutants were found that weren't caught by tests.

- **1**: Usage error: bad command-line arguments etc.

- **2**: Found some mutants that were not covered by tests.

- **3**: Some tests timed out: possibly the mutatations caused an infinite loop,
  or the timeout is too low.

- **4**: The tests are already failing or hanging before any mutations are
  applied, so no mutations were tested.

### `mutants.out`

A `mutants.out` directory is created in the source directory. It contains:

- A `logs/` directory, with one log file for each mutation plus the baseline
  unmutated case. The log contains the diff of the mutation plus the output from
  cargo.

- A `lock.json`, on which an [fs2 lock](https://docs.rs/fs2) is held while
  cargo-mutants is running, to avoid two tasks trying to write to the same
  directory at the same time. The lock contains the start time, cargo-mutants
  version, username, and hostname. `lock.json` is left in `mutants.out` when the
  run completes, but the lock on it is released.

- A `mutants.json` file describing all the generated mutants.

- An `outcomes.json` file describing the results of all tests.

### Hangs and timeouts

Some mutations to the tree can cause the test suite to hang. For example, in
this code, cargo-mutants might try changing `should_stop` to always return
`false`:

```rust
    while !should_stop() {
      // something
    }
```

`cargo mutants` automatically sets a timeout when running tests with mutations
applied, and reports mutations that hit a timeout. The automatic timeout is the
maximum of 20 seconds, or 5x the time to run tests with no mutations.

You can also set an explicit timeout with the `--timeout` option. In this case
the timeout is also applied to tests run with no mutation.

The timeout does not apply to `cargo check` or `cargo build`, only `cargo test`.

When a test times out, you can mark it with `#[mutants::skip]` so that future
`cargo mutants` runs go faster.

### Performance

Most of the runtime for cargo-mutants is spent in running the program test suite
and in running incremental builds: both are done once per viable mutant.

So, anything you can do to make the `cargo build` and `cargo test` suite faster
will have a multiplicative effect on `cargo mutants` run time, and of course
will also make normal development more pleasant.

There's lots of good advice on the web, including <https://matklad.github.io/2021/09/04/fast-rust-builds.html>.

In particular, on Linux, using the [Mold linker](https://github.com/rui314/mold)
can improve build times significantly: because cargo-mutants does many
incremental builds, link time is important.

Rust doctests are pretty slow, so if you're using them only as testable
documentation and not to assert correctness of the code, you can skip them with
`cargo mutants -- --all-targets`.

### Hard-to-test cases

Some functions don't cause a test suite failure if emptied, but also cannot be
removed. For example, functions to do with managing caches or that have other
performance side effects.

Ideally, these should be tested, but doing so in a way that's not flaky can be
difficult. cargo-mutants can help in a few ways:

- It helps to at least highlight to the developer that the function is not
  covered by tests, and so should perhaps be treated with extra care, or tested
  manually.
- A `#[mutants::skip]` annotation can be added to suppress warnings and explain
  the decision.
- Sometimes these effects can be tested by making the side-effect observable
  with, for example, a counter of the number of memory allocations or cache
  misses/hits.

### Continuous integration

Here is an example of a GitHub Actions workflow that runs mutation tests and uploads the results as an artifact. This will fail if it finds any uncaught mutants.

```yml
name: cargo-mutants

on: [pull_request, push]

jobs:
  cargo-mutants:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Install cargo-mutants
        run: cargo install cargo-mutants
      - name: Run mutant tests
        run: cargo mutants -- --all-features
      - name: Archive results
        uses: actions/upload-artifact@v3
        if: failure()
        with:
          name: mutation-report
          path: mutants.out
```

## How to help

Experience reports in GitHub Discussions or Bugs are very welcome:

- Did it find a bug or important coverage gap?
- Did it fail to build and test your tree? (Some cases that aren't supported yet
  are already listed in this doc or the bug tracker.)

It's especially helpful if you can either point to an open source tree that will
reproduce the problem (or success) or at least describe how to reproduce it.

## Goals

**The goal of cargo-mutants is to be _easy_ to run on any Rust source tree, and
to tell you something _interesting_ about areas where bugs might be lurking or
the tests might be insufficient.**

Being _easy_ to use means:

- cargo-mutants requires no changes to the source tree or other setup: just
  install and run. So, if it does not find anything interesting to say about a
  well-tested tree, it didn't cost you much. (This worked out really well:
  `cargo install cargo-mutants && cargo mutants` will do it.)

- There is no chance that running cargo-mutants will change the released
  behavior of your program (other than by helping you to fix bugs!), because you
  don't need to change the source to use it.

- cargo-mutants should be reasonably fast even on large Rust trees. The overall
  run time is, roughly, the product of the number of viable mutations multiplied
  by the time to run the test suite for each mutation. Typically, one `cargo
  mutants` run will give you all the information it can find about missing test
  coverage in the tree, and you don't need to run it again as you iterate on
  tests, so it's relatively OK if it takes a while. (There is currently very
  little overhead beyond the cost to do an incremental build and run the tests
  for each mutant, but that can still be significant for large trees. There's
  room to improve by testing multiple mutants in parallel.)

- cargo-mutants should run correctly on any Rust source trees that are built and
  tested by Cargo, that will build and run their tests in a copy of the tree,
  and that have hermetic tests. (It's not all the way there yet; in particular
  it assumes the source is in `src/`.)

- cargo-mutants shouldn't crash or hang, even if it generates mutants that cause
  the software under test to crash or hang. (This is generally met today:
  cargo-mutants runs tests with an automatically set and configurable timeout.)

- The results should be reproducible, assuming the build and test suite is
  deterministic. (This should be true today; please file a bug if it's not. Mutants are run in random order unless `--no-shuffle` is specified, but this should not affect the results.)

- cargo-mutants should avoid generating unviable mutants that don't compile,
  because that wastes time. However, when it's uncertain whether the mutant will
  build, it's worth trying things that _might_ find interesting results even if
  they might fail to build.  (It does currently generate _some_ unviable mutants, but typically not too many, and they don't have a large effect on runtime in most trees.)

- Realistically, cargo-mutants may generate some mutants that aren't caught by
  tests but also aren't interesting, or aren't feasible to test. In those cases
  it should be easy to permanently dismiss them (e.g. by adding a
  `#[mutants::skip]` attribute or a config file.) (The attribute exists but
  there is no config file yet.)

Showing _interesting results_ mean:

- cargo-mutants should tell you about places where the code could be wrong and
  the test suite wouldn't catch it. If it doesn't find any interesting results
  on typical trees, there's no point. Aspirationally, it will even find useful
  results in code with high line coverage, when there is code that is reached by
  a test, but no test depends on its behavior.

- In superbly-tested projects cargo-mutants may find nothing to say, but hey, at
  least it was easy to run, and hopefully the assurance that the tests really do
  seem to be good is useful data.

- _Most_, ideally all, findings should indicate something that really should be
  tested more, or that may already be buggy, or that's at least worth looking at.

- It should be easy to understand what the output is telling you about a
  potential bug that wouldn't be caught. (This seems true today.) It might take
  some thought to work out _why_ the existing tests don't cover it, or how to
  check it, but at least you know where to begin.

- As much as possible cargo-mutants should avoid generating trivial mutants,
  where the mutated code is effectively equivalent to the original code, and so
  it's not interesting that the test suite doesn't catch the change. (Not much
  has been done here yet.)

- For trees that are thoroughly tested, you can use `cargo mutants` in CI to
  check that they remain so.

## How it works

The basic approach is:

- First, run `cargo build --tests` in the source tree to
  "freshen" it so that the mutated copies will have a good starting point. (This
  is skipped with `--no-copy-target`.)

- Make a copy of the source tree into a scratch directory, excluding
  version-control directories like `.git` and optionally excluding the `/target`
  directory. The same directory is reused across all the mutations to benefit
  from incremental builds.

  - After copying the tree, cargo-mutants scans the top-level `Cargo.toml` and any
    `.cargo/config.toml` for relative dependencies. If there are any, the paths are
    rewritten to be absolute, so that they still work when cargo is run in the
    scratch directory.

  - Before applying any mutations, check that `cargo test` succeeds in the
    scratch directory: perhaps a test is already broken, or perhaps the tree
    doesn't build when copied because it relies on relative paths to find
    dependencies, etc.

- Build a list of mutations:
  - Run `cargo metadata` to find directories containing Rust source files.
  - Walk all source files and parse each one looking for functions.
  - Skip functions that should not be mutated for any of several reasons:
    because they're tests, because they have a `#[mutants::skip]` attribute,
    etc.
  - For each function, depending on its return type, generate every mutation
    pattern that produces a result of that type.

- For each mutation:
  - Apply the mutation to the scratch tree by patching the affected file.
  - Run `cargo test` in the tree, saving output to a log file.
  - If the build fails or the tests fail, that's good: the mutation was somehow
    caught.
  - If the build and tests succeed, that might mean test coverage was
    inadequate, or it might mean we accidentally generated a no-op mutation.
  - Revert the mutation to return the tree to its clean state.

The file is parsed using the [`syn`](https://docs.rs/syn) crate, but mutations
are applied textually, rather than to the token stream, so that unmutated code
retains its prior formatting, comments, line numbers, etc. This makes it
possible to show a text diff of the mutation and should make it easier to
understand any error messages from the build of the mutated code.

For more details, see [DESIGN.md](DESIGN.md).

## Related work

cargo-mutants was inspired by reading about the
[Descartes mutation-testing tool for Java](https://github.com/STAMP-project/pitest-descartes/)
described in
[Increment magazine's testing issue](https://increment.com/reliability/testing-beyond-coverage/).

It's an interesting insight that mutation at the level of a whole function is a
practical sweet-spot to discover missing tests, while still making it feasible
to exhaustively generate every mutant, at least for moderate-sized trees.

See also: [more information on how cargo-mutants compares to other techniques and tools](https://github.com/sourcefrog/cargo-mutants/wiki/Compared).

## Supported Rust versions

Building cargo-mutants requires a recent stable Rust toolchain.

Currently it is [tested with 1.58](https://github.com/sourcefrog/cargo-mutants/actions/workflows/msrv.yml).

After installing cargo-mutants, you should be able to use it to run tests under
any toolchain, even toolchains that are too old to build cargo-mutants, using the standard `+` option to `cargo`:

```sh
cargo +1.48 mutants
```

### Limitations, caveats, known bugs, and future enhancements

cargo-mutants behavior, output formats, command-line syntax, json output
formats, etc, may change from one release to the next.

- cargo-mutants does not yet understand cargo workspaces, and it will only test the root package. <https://github.com/sourcefrog/cargo-mutants/issues/45>

- cargo-mutants sees the AST of the tree but doesn't fully "understand" the
  types. Possibly it could learn to get type information from the compiler (or
  rust-analyzer?), which would help it generate more interesting viable mutants,
  and fewer unviable mutants.

- To make this faster on large trees, we could keep several scratch trees and
  test them in parallel, which is likely to exploit CPU resources more
  thoroughly than Cargo's own parallelism: in particular Cargo tends to fall
  down to a single task during linking, and often comes down to running a single
  straggler test at a time. <https://github.com/sourcefrog/cargo-mutants/issues/39>

## Code of Conduct

Interaction with or participation in this project is governed by the [Rust Code
of Conduct](https://www.rust-lang.org/policies/code-of-conduct).

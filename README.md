# cargo-mutants

<https://github.com/sourcefrog/cargo-mutants>

[![Tests](https://github.com/sourcefrog/cargo-mutants/actions/workflows/tests.yml/badge.svg?branch=main&event=push)](https://github.com/sourcefrog/cargo-mutants/actions/workflows/tests.yml?query=branch%3Amain)
[![crates.io](https://img.shields.io/crates/v/cargo-mutants.svg)](https://crates.io/crates/cargo-mutants)
[![libs.rs](https://img.shields.io/badge/libs.rs-cargo--mutants-blue)](https://lib.rs/crates/cargo-mutants)
![Maturity: Beta](https://img.shields.io/badge/maturity-beta-blue.svg)

cargo-mutants is a mutation testing tool for Rust. It guides you to missing test
coverage by finding functions whose implementation could be replaced by
something trivial without causing any tests to fail.

Coverage measurements can be helpful, but they really tell you what code is
_reached_ by a test, and not whether the test really _checks_ anything about the
behavior of the code. Mutation tests give different, perhaps richer information,
about whether the tests really check the code's behavior.

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
% cargo mutants --dir ~/src/unix_mode/
baseline test with no mutations ... ok
replace type_bits with Default::default() in src/lib.rs:42:32 ... caught
replace is_file with Default::default() in src/lib.rs:52:35 ... caught
replace is_dir with Default::default() in src/lib.rs:62:34 ... caught
replace is_symlink with Default::default() in src/lib.rs:72:38 ... caught
replace is_fifo with Default::default() in src/lib.rs:77:35 ... caught
replace is_char_device with Default::default() in src/lib.rs:82:42 ... caught
replace is_block_device with Default::default() in src/lib.rs:87:43 ... NOT CAUGHT!
...
```

In this version of the `unix_mode` crate, the `is_block_device` function was
indeed untested.

To see what mutants could be generated without running them, use `--list`.
`--list` also supports a `--json` option to make the output more
machine-readable, and a `--diff` option to show the replacement.

### Understanding the results

If tests fail in a clean copy of the tree, there might be an (intermittent)
failure in the source directory, or there might be some problem that stops them
passing when run from a different location, such as a relative `path` in
`Cargo.toml`. Fix this first.

Otherwise, cargo mutants generates every mutant it can. All mutants fall in to
one of these categories:

- **caught** — A test failed with this mutant applied. This is a good sign about
  test coverage. You can look in `mutants.out/log` to see which tests failed.

- **not caught** — No test failed with this mutation applied, which seems to
  indicate a gap in test coverage. Or, it may be that the mutant is
  undistinguishable from the correct code. You may wish to add a better test, or
  mark that the function should be skipped.

- **check failed** — `cargo check` failed on the mutated code, probably because
  the mutation does not typecheck. This is inconclusive about test coverage and
  no action is needed, but indicates an opportunity for cargo-mutants to either
  generate better mutants, or at least not generate unviable mutants.

- **build failed** — Similarly, but `cargo build` failed. This should be rare.

By default only "not caught" mutants are printed; others can be shown with the
`-v` and `-V` options.

### Skipping functions

To mark functions so they are not mutated:

1. Add a Cargo dependency on the [mutants](https://crates.io/crates/mutants)
   crate. (Note that this must be a regular `dependency` not a `dev-dependency`,
   because the annotation will be on non-test code.)

2. Mark functions with `#[mutants::skip]`.

The crate is tiny and the attribute has no effect on the compiled code. It only
flags the function for cargo-mutants.

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

- A `mutants.json` file describing all the generated mutants.

- An `outcomes.json` file describing the results of all tests.

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
maximum of 5 seconds, or 3x the time to run tests with no mutations.

You can also set an explicit timout with the `--timeout` option. In this case
the timeout is also applied to tests run with no mutation.

The timeout does not apply to `cargo check` or `cargo build`, only `cargo test`.

When a test times out, you can mark it with `#[mutants::skip]` so that future
`cargo mutants` runs go faster.

### Tips

- Trees that `deny` style lints such as unused parameters are likely to fail to
  build when mutated, without really saying much about the value of the tests. I
  suggest you don't statically deny warnings in your source code, but rather set
  `RUSTFLAGS` when you do want to check this — and don't do this when running
  `cargo mutants`.

### Performance

Anything you can do to make the `cargo build` and `cargo test` suite faster will
have a multiplicative effect on `cargo mutants` run time, and of course will
also make normal development more pleasant. There's lots of good advice on the
web.

In particular, on Linux, using the [Mold linker](https://github.com/rui314/mold)
can improve build times significantly: because cargo-mutants does many
incremental builds, link time is important.

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

### Other options

`--no-copy-target` skips building the source tree and excludes `/target` from
copying. The effect of this is that the first, baseline, build in the scratch
directory will be a "clean" build with nothing in `/target`. This will typically
be slower (which is why `/target` is copied by default) but it might help in
debugging any issues with the build. (And, in niche cases where there is a very
large volume of old unreferenced content in `/target`, it might conceivably be
faster, but that's probably better dealt with by `cargo clean` in the source
directory.)

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
  deterministic. (This should be true today; please file a bug if it's not.)

- cargo-mutants should avoid generating unviable mutants that don't compile,
  because that wastes time. However, when it's uncertain whether the mutant will
  build, it's worth trying things that _might_ find interesting results even if
  they might fail to build.  (There is room for improvement here too, but since
  cargo-mutants runs a cheap `cargo check` on each mutant first, the cost to try
  unviable mutants is actually not too bad. Most of the time is in running the
  tests for viable mutants.)

- Realistically, cargo-mutants may generate some mutants that aren't caught by
  tests but also aren't interesting, or aren't feasible to test. In those cases
  it should be easy to permanently dismiss them (e.g. by adding a
  `#[mutants::skip]` attribute or a config file.) (The attribute exists but
  there is no config file yet.)

Showing _interesting results_ mean:

- cargo-mutants should tell you about places where the code could be wrong and
  the test suite wouldn't catch it. If it doesn't find any interesting results
  on typical trees, there's no point.

- _Most_, ideally all, findings should indicate something that really should be
  tested more, or that may already be buggy, or that's at least worth looking at.

- Mutation testing complements coverage tools by finding code that might be
  executed by a test (and show up as covered) but where the test result does not
  actually _depend on_ the behavior of the code, which can happen pretty easily.

- Mutation testing also complements fuzzing or property testing by covering code
  that might be hard to hook up to a fuzzer interface, or where that work just
  has not been done yet.

- It should be easy to understand what the output is telling you. It may take some
  thought about how to effectively test the under-tested code, but at least it's
  easy to see the potential bug that wouldn't be caught.

- Although run time matters, it's worth spending more time to generate more
  mutants that might find interesting results, since developers should not need
  to run this and wait for results the way they might run their normal test
  suite. (There's lots of scope to burn more CPU cycles by adding more mutation
  patterns.)

- As much as possible cargo-mutants should avoid generating trivial mutants,
  where the mutated code is effectively equivalent to the original code, and so
  it's not interesting that the test suite doesn't catch the change. (Not much
  has been done here yet.)

- On trees that are already very well-tested, cargo-mutants may find nothing
  interesting, and then it should just say so. (This seems to be true today.)

- And for trees that are thoroughly tested, you can use `cargo mutants` in CI to
  check that they remain so.

## How it works

The basic approach is:

- First, run `cargo build --tests` and `cargo check` in the source tree to
  "freshen" it so that the mutated copies will have a good starting point. (This
  is skipped with `--no-copy-target`.)

- Make a copy of the source tree into a scratch directory, optionally excluding
  the `/target` directory. The same directory is reused across all the mutations
  to benefit from incremental builds.

  - Before applying any mutations, check that `cargo test` succeeds in the
    scratch directory: perhaps a test is already broken, or perhaps the tree
    doesn't build when copied because it relies on relative paths to find
    dependencies, etc.

- Build a list of mutations:
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
    (Doing so is a shortcoming in this tool.)
  - Revert the mutation to return the tree to its clean state.

The nice thing about `Default` is that it's defined on many commonly-used types
including `()`, so cargo-mutants does not need to really understand the function
return type at this early stage. Some functions will fail to build because they
return a type that does not implement `Default`, and that's OK.

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
practical sweet-spot to discover missing tests, while (at least at moderate-size
trees) still making it feasible to exhaustively generate every mutant.

### Mutagen

There's an existing Rust mutation testing tool called
[Mutagen](https://github.com/llogiq/mutagen).

Some differences are:

- Mutagen seems more mature.

- Mutagen requires changes to the source tree, and for functions to be mutated
  to be marked with an attribute. cargo-mutants can work with any unmodified
  tree.

- Mutagen builds the tree only once; cargo-mutants does an incremental build for
  each mutation.

  This is slower, although for some trees the incremental build may be
  relatively cheap compared to running the test suite.

  On the up side building for each mutation gives cargo-mutants the freedom to
  try mutations it's not sure will compile.

- Mutagen has a neat system to use coverage information to run only the tests
  that could possibly be affected. This could potentially be ported.

- Mutagen needs a nightly compiler. cargo-mutants can be built with any recent
  stable (or nightly) compiler and can be used with very old compilers.

Please let me know anything else that should be added or corrected.

## Supported Rust versions

Building cargo-mutants requires a recent stable Rust toolchain.

After installing cargo-mutants, you should be able to use it to run tests under
any toolchain, using the standard `+` option to `cargo`:

```sh
cargo +1.48 mutants
```

## Project status

cargo-mutants behavior, output formats, command-line syntax, json output
formats, etc, may change from one release to the next.

The repertoire of generated mutants is smaller than it could be, and
cargo-mutants generates and tries to build some mutants that it really should
know won't work out.

## Limitations, caveats, known bugs, and future enhancements

See also the list of goals, above, which has some commentary on where
cargo-mutants is not yet meeting its goals.

- cargo-mutants sees the AST of the tree but doesn't fully "understand" the
  types. Possibly it could learn to get type information from the compiler (or
  rust-analyzer?), which would help it generate more interesting viable mutants,
  and fewer unviable mutants.

- Copying the tree to build it doesn't work well if the `Cargo.toml` points to
  dependencies by a relative `path` (other than in subdirectories). This could
  be handled by an option to mutate in-place (maybe into a copy made by the
  user) or possibly an option to copy a larger containing directory. You can
  work around this by editing `Cargo.toml` to make the paths absolute, before
  running `cargo mutants`

- To make this faster on large trees, we could keep several scratch trees and
  test them in parallel, which is likely to exploit CPU resources more
  thoroughly than Cargo's own parallelism: in particular Cargo tends to fall
  down to a single task during linking, and often comes down to running a single
  straggler test at a time.

- cargo-mutants currently assumes all the source is in `src/` of the directory, but Cargo
  doesn't require that, and some crates have their source in a different
  directory. This could be fixed by reading `cargo metadata`.
  <https://github.com/sourcefrog/cargo-mutants/issues/29>

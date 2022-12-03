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

### Command-line options

`-d`, `--dir`: Test the Rust tree in the given directory, rather than the default directory.

`--list`: Show what mutants could be generated, without running them.

`--diff`: With `--list`, also include a diff of the source change for each mutant.

`--jobs`: Run this many jobs in parallel.

`--json`: With `--list`, show the list in json.

`--check`: Run `cargo check` on all generated mutants to find out which ones are viable, but don't actually run the tests.

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

### Filtering files

Two options (each with short and long names) control which files are mutated:

`-f GLOB`, `--file GLOB`: Mutate only functions in files matching
the glob.

`-e GLOB`, `--exclude GLOB`: Exclude files that match the glob.

These options may be repeated.

If any `-f` options are given, only source files that match are
considered; otherwise all files are considered. This list is then further
reduced by exclusions.

If the glob contains `/` (or on Windows, `\`), then it matches against the path from the root of the source
tree. For example, `src/*/*.rs` will exclude all files in subdirectories of `src`.

If the glob does not contain a path separator, it matches against filenames
in any directory.

`/` matches the path separator on both Unix and Windows.

Note that the glob must contain `.rs` (or a matching wildcard) to match
source files with that suffix. For example, `-f network` will match
`src/network/mod.rs` but it will _not_ match `src/network.rs`.

Files that are excluded are still parsed (and so must be syntactically
valid), and `mod` statements in them are followed to discover other
source files. So, for example, you can exclude `src/main.rs` but still
test mutants in other files referenced by `mod` statements in `main.rs`.

The results of filters can be previewed with the `--list-files` and `--list`
options.

Examples:

- `cargo mutants -f visit.rs -f change.rs` -- test mutants only in files
  called `visit.rs` or `change.rs` (in any directory).

- `cargo mutants -e console.rs` -- test mutants in any file except `console.rs`.

- `cargo mutants -f src/db/*.rs` -- test mutants in any file in this directory.

### Filtering functions and mutants

Two options filter mutants by the full name of the mutant, which includes the
function name, file name, and a description of the change.

Mutant names are shown by `cargo mutants --list`, and the same command can be
used to preview the effect of filters.

`-F REGEX`, `--re REGEX`: Only test mutants whose full name matches the given regex.

`-E REGEX`, `--exclude-re REGEX`: Exclude mutants whose full name matches
the given regex.

These options may be repeated.

The regex matches a substring and can be anchored with `^` and `$`.

The regex syntax is defined by the [`regex`](https://docs.rs/regex/latest/regex/)
crate.

These filters are applied after filtering by filename, and `--re` is applied before
`--exclude-re`.

Examples:

- `-E 'impl Debug'` -- don't test `impl Debug` methods, because coverage of them
  might be considered unimportant.

- `-F 'impl Serialize' -F 'impl Deserialize'` -- test implementations of these
  two traits.

### Workspace and package support

cargo-mutants now supports testing Cargo workspaces that contain multiple packages.

All source files in all packages in the workspace are tested. For each mutant, only the containing packages tests are run.

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

## How to help

Experience reports in [GitHub Discussions](https://github.com/sourcefrog/cargo-mutants/discussions) or issues are very welcome:

- Did it find a bug or important coverage gap?
- Did it fail to build and test your tree? (Some cases that aren't supported yet
  are already listed in this doc or the bug tracker.)

It's especially helpful if you can either point to an open source tree that will
reproduce the problem (or success) or at least describe how to reproduce it.

If you are interested in contributing a patch, please read [CONTRIBUTING.md](CONTRIBUTING.md).

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

- Make a copy of the source tree into a scratch directory, excluding
  version-control directories like `.git` and the `/target` directory. The same directory is reused across all the mutations to benefit from incremental builds.

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

### Limitations, caveats, known bugs, and future enhancements

**CAUTION**: This tool builds and runs code with machine-generated
modifications. If the code under test, or the test suite, has side effects such
as writing or deleting files, running it with mutations may be dangerous. Think
first about what side effects the test suite could possibly have, and/or run it
in a restricted or disposable environment.

cargo-mutants behavior, output formats, command-line syntax, json output
formats, etc, may change from one release to the next.

cargo-mutants sees the AST of the tree but doesn't fully "understand" the types.
Possibly it could learn to get type information from the compiler (or
rust-analyzer?), which would help it generate more interesting viable mutants,
and fewer unviable mutants.

cargo-mutants reads `CARGO_ENCODED_RUSTFLAGS` and `RUSTFLAGS` environment variables, and sets `CARGO_ENCODED_RUSTFLAGS`.  It does not read `.cargo/config.toml` files, and so any rust flags set there will be ignored.

## Integrations and related work

### vim-cargomutants

[`vim-cargomutants`](https://github.com/yining/vim-cargomutants) provides commands
view cargo-mutants results, see the diff of mutations, and to launch cargo-mutants
from within vim.

## Code of Conduct

Interaction with or participation in this project is governed by the [Rust Code
of Conduct](https://www.rust-lang.org/policies/code-of-conduct).

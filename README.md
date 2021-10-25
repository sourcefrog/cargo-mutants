# cargo-mutants

https://github.com/sourcefrog/cargo-mutants

[![Tests](https://github.com/sourcefrog/cargo-mutants/actions/workflows/tests.yml/badge.svg)](https://github.com/sourcefrog/cargo-mutants/actions/workflows/tests.yml)

cargo-mutants is a mutation testing tool for Rust. It guides you to missing
test coverage by finding functions whose implementation could be replaced by
something trivial and the tests would all still pass. 

Coverage measurements can be helpful, but they really tell you what code is
_reached_ by a test, and not whether the test really assert anything about the
behavior of the code.

**CAUTION**: This tool builds and runs code with machine-generated
modifications. If the code under test, or the test suite, has side effects such
as writing or deleting files, running it with mutations may be dangerous: for
example it might write to or delete files outside of the source tree.
Eventually, cargo-mutants might support running tests inside a jail or sandbox.
For now, think first about what side effects the test suite could possibly have
and/or run it in a restricted or disposable environment.

**NOTE:** cargo-mutants is still pretty new! It can find some interesting
results, but because it has a very basic idea of which functions to mutate and
how, it generates significant false positives and false negatives. The
proof-of-concept is successful, though, and I think the results can be
iteratively improved.

## Install

    cargo install cargo-mutants

## Using cargo-mutants

Just run `cargo mutants` in a Rust source directory, and it will point out
functions that may be inadequately tested:

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

The Cargo output is logged into `target/mutants/` within the original source
directory, so you can see why individual tests failed.

### Skipping functions

To mark functions so they are not mutated:

1. Add a Cargo dependency on the [mutants](https://crates.io/crates/mutants) crate.

2. Mark functions with `#[mutants::skip]`.

The crate is tiny and the attribute has no effect on the compiled code. It only
flags the function for cargo-mutants.

### Tips

* Trees that `deny` style lints such as unused parameters are likely to fail to
  build when mutated, without really saying much about the value of the tests.
  I suggest you don't statically deny warnings in your source code, but rather
  set `RUSTFLAGS` when you do want to check this.

## Limitations, caveats, and known bugs

* Some mutations will cause the program to hang or spin, for example if called
  as the condition of a `while` loop. cargo-mutants currently detects this but does
  not kill the test process, so you'll need to find and kill it yourself. (On
  Unix we might need to use `setpgrp`.)

* Copying the tree to build it doesn't work well if the `Cargo.toml` points to
  dependencies by a relative `path`.

## Goals

* Draw attention to code that is not tested or only "pseudo-tested": reached by tests but the tests
  don't actually depend on the behavior of the function.

* Be fast enough to plausibly run on thousand-file trees, although not necessarily instant. It's OK if
  it takes some minutes to run.

* Easy start: no annotations need to be added to the source tree to use cargo-mutants, 
  you just run it and it should say something useful about any Rust tree.  
  I'd like it to be useful on a tree you haven't seen before, where you're not
  sure of the quality of the tests.

* Understandable output including easily reproducible test failures and a copy of the output.

* Signal to noise: most warnings should indicate something that really should
  be tested more; false positives should be easy to durably dismiss. It ought
  to be feasible to run it from CI to produce a reliable warning that
  inadequately-tested code is being added, without being too annoying.

* Opt-in annotations to disable mutation of some methods or to explain how to mutate them usefully.

* Cause no changes to the release build.

* Rust only, Cargo only.

## Limitations, caveats, and known bugs

* In this version, the _only_ mutation it applies is to return
  `Default::default()`. For many functions, and in particular for the common
  case of returning a `Result`, this will fail to build, which is not an
  interesting result. cargo-mutants can see the return type in the tree, so
  it's possible to do much better by returning `Ok(Default::default())` and so
  on for other return types. I expect to fix this soon.

* It should skip functions with `#[cfg(...)]` attributes that don't match the
  current platform, but it does not yet.

* It should also probably skip `unsafe` functions, and maybe functions
  containing `unsafe {}` blocks.

* Some mutations will cause the program to hang or spin, for example if called
  as the condition of a `while` loop. Enucleate currently detects this but does
  not kill the test process, so you'll need to find and kill it yourself. (On
  Unix we might need to use `setpgrp`.)

* Trees that `deny` style lints such as unused parameters are likely to fail to
  build when mutated, without really saying much about the value of the tests.
  I suggest you don't statically deny warnings in your source code, but rather
  set `RUSTFLAGS` when you do want to check this.

* To make this faster on large trees, we could keep several scratch trees and
  test them in parallel, which is likely to exploit CPU resources more
  thoroughly than Cargo's own parallelism: in particular Cargo tends to fall
  down to a single task during linking.

* It currently assumes all the source is in `src/` of the directory, but Cargo
  doesn't require that, and some crates have their source in a different
  directory. This could be fixed by reading `cargo metadata`.

## Hard-to-test cases

Some functions don't cause a test suite failure if emptied, but also cannot be
removed. For example, functions to do with managing caches or that have other
performance side effects.

Ideally, these should be tested, but doing so in a way that's not flaky can be
difficult. cargo-mutants can help in a few ways:

* It helps to at least highlight to the developer that the function is not covered by tests, and
  so should perhaps be treated with extra care, or tested manually.
* A `#[mutants::skip]` annotation can be added to suppress warnings and explain the decision.
* Sometimes these effects can be tested by making the side-effect observable with, for example,
  a counter of the number of memory allocations or cache misses/hits.

## Approach

The basic approach is:

* Make a copy of the whole tree into a scratch directory. The same directory is reused 
  across all the mutations to benefit from incremental builds.
* Build a list of possible mutations and for each one:
  * Apply the mutation to the scratch tree.
  * Run `cargo test` in the tree, saving output to a log file.
  * If the build fails or the tests fail, that's good: the mutation was somehow caught.
  * If the build and tests succeed, that might mean test coverage was inadequate, or it might mean
    we accidentally generated a no-op mutation. (Doing so is a shortcoming in this tool.)

The list of possible mutations is generated by:

* Walk all source files and enumerate all item (top-level) and method functions. For each one:
   * Filter functions that should not be mutated:
      * Already-trivial functions whose result would not be changed by mutation. 
      * Functions marked with an `#[mutants::skip]` attribute.
      * Functions marked `#[test]`.
      * Functions marked `#[cfg(test)]` or inside a `mod` so marked.
   * Apply every supported mutation:
      * Replace the body with `Default::default()`.
        * If the function returns a `Result`, instead replace the body with `Ok(Default::default())`.
      * (maybe more in future)

The nice thing about `Default` is that it's defined on many commonly-used types including `()`,
so cargo-mutants does not need to really understand the function return type at this early stage.
Some functions will fail to build because they return a type that does not implement `Default`,
and that's OK.

The file is parsed using the [`syn`](https://docs.rs/syn) crate, but mutations
are applied textually, rather than to the token stream, so that unmutated code
retains its prior formatting, comments, line numbers, etc. This makes it
possible to show a text diff of the mutation and should make it easier to
understand any error messages from the build of the mutated code.

## Related work

cargo-mutants was  inspired by reading about the [Descartes mutation-testing
tool for Java](https://github.com/STAMP-project/pitest-descartes/) described in
[Increment magazine's testing
issue](https://increment.com/reliability/testing-beyond-coverage/).  

It's an interesting insight that mutation at the level of a whole function is a
practical sweet-spot to discover missing tests, while (at least at
moderate-size trees) still making it feasible to exhaustively generate every
mutant.

### Mutagen

There's an existing Rust mutation testing tool called [Mutagen](https://github.com/llogiq/mutagen).

Some differences are:

* Mutagen seems more mature.

* Mutagen requires changes to the source tree, and for functions to be mutated to be marked with an attribute. cargo-mutants can work with any unmodified tree.

* Mutagen builds the tree only once; cargo-mutants does an incremental build for each mutation. 

  This is slower, although for some trees the incremental build may be relatively cheap compared to running the test suite.

  On the up side building for each mutation gives cargo-mutants the freedom to try mutations it's not sure will compile.

* Mutagen has a neat system to use coverage information to run only the tests that could possibly be affected. This could potentially be ported.

* Mutagen needs a nightly compiler; cargo-mutants should work with any reasonably-recent compiler and is tested on stable.

* (Probably there are more. Please point out any errors.)

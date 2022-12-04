# Using the results

## Tests fail in an clean tree?

If tests fail in a clean copy of the tree, there might be an (intermittent)
failure in the source directory, or there might be some problem that stops them
passing when run from a different location. Fix this first: cargo-mutants can't do anything until you have a tree where `cargo test` passes reliably when copied to a temporary directory.

## Mutant outcomes

Assuming tests pass in a clean copy of the tree, cargo-mutants proceeds to generate every mutant it can, subject to any configured filters, and then runs `cargo build` and `cargo test` on each of them.

Each mutant results in one of the following outcomes:

* **caught** — A test failed with this mutant applied. This is a good sign about
  test coverage.

* **missed** — No test failed with this mutation applied, which seems to
  indicate a gap in test coverage. Or, it may be that the mutant is
  undistinguishable from the correct code. You may wish to add a better test, or
  mark that the function should be skipped.

* **unviable** — The attempted mutation doesn't compile. This is inconclusive about test coverage and
  no action is needed, but indicates an opportunity for cargo-mutants to either
  generate better mutants, or at least not generate unviable mutants.

* **timeout** — The mutation caused the test suite to run for a long time, until it was eventually killed. You might want to investigate the cause and potentially mark the function to be skipped.

By default only missed mutants and timeouts are printed to stdout, because they're the most actionable. Others can be shown with the `--caught` and `--unviable` options.

## What to do about missed mutants?

Each missed mutant is a sign that there _might_ be a gap in test coverage. What
to do about them is up to you, bearing in mind your goals and priorities for
your project, but here are some suggestions:

First, look at the overall list of missed mutants: there might be patterns such
as a cluster of related functions all having missed mutants. Probably some will
stand out as potentially more important to the correct function of your program.

You should first look for any mutations where it's very _surprising_ that they
were not caught by any tests, given what you know about the codebase. For
example, if cargo-mutants reports that replacing an important function with
`Ok(())` is not caught then that seems important to investigate.

You should then look at the tests that you would think _would_ catch the mutant:
that might be unit tests within the relevant module, or some higher-level
public-API or integration test, depending on how your project's tests are
structured.

If you can't find any tests that you think should have caught the mutant, then
perhaps you should add some. The right thing here is _not_ necessarily to
directly assert that the mutated behavior doesn't happen. For example, if the
mutant changed a private function, you don't necessarily want to add a test for
that private function, but instead ask yourself what public-API behavior would
break if the private function was buggy, and then add a test for that.

Try to avoid writing tests that are too tightly targeted to the mutant, which is
really just an _example_ of something that could be wrong, and instead write
tests that assert the _correct_ behavior at the right level of abstraction,
preferably through a public interface.

If it's not clear why the tests aren't already failing, it may help to manually
inject the same mutation into your working tree and then run the tests under a
debugger, or add trace statements to the test. (The `--diff` option or looking
in the `mutants.out` directory will show you exactly what change cargo-mutants
made.)

You may notice some messages about missed mutants in functions that you feel are
not very important to test, such as `Debug` implementations. You can use the
 `--exclude-re` options to filter out these mutants, or mark them as
skipped with `#[mutants::skip]`. (Or, you might decide that you do want to add
unit tests for the `Debug` representation, but perhaps as a lower priority than
investigating mutants in more important code.)

In some cases cargo-mutants will generate a mutant that is effectively the same as the original code, and so not really incorrect. cargo-mutants tries to avoid doing this, but if it does happen then you can mark the function as skipped.

## Iterating on mutant coverage

After you've changed your program to address some of the missed mutants, you can
run `cargo mutants` again with the [`--file` option](skip_files.md) to re-test
only functions from the changed files.

## Hard-to-test cases

Some functions don't cause a test suite failure if emptied, but also cannot be
removed. For example, functions to do with managing caches or that have other
performance side effects.

Ideally, these should be tested, but doing so in a way that's not flaky can be
difficult. cargo-mutants can help in a few ways:

* It helps to at least highlight to the developer that the function is not
  covered by tests, and so should perhaps be treated with extra care, or tested
  manually.
* A [`#[mutants::skip]` annotation](skip.md) can be added to suppress warnings
  and explain the decision.
* Sometimes these effects can be tested by making the side-effect observable
  with, for example, a counter of the number of memory allocations or cache
  misses/hits.

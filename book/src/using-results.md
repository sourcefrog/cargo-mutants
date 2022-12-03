# Using the results

If tests fail in a clean copy of the tree, there might be an (intermittent)
failure in the source directory, or there might be some problem that stops them
passing when run from a different location, such as a relative `path` in
`Cargo.toml`. Fix this first.

Otherwise, cargo mutants generates every mutant it can. All mutants fall in to
one of these categories:

* **caught** — A test failed with this mutant applied. This is a good sign about
  test coverage. You can look in `mutants.out/log` to see which tests failed.

* **missed** — No test failed with this mutation applied, which seems to
  indicate a gap in test coverage. Or, it may be that the mutant is
  undistinguishable from the correct code. You may wish to add a better test, or
  mark that the function should be skipped.

* **unviable** — The attempted mutation doesn't compile. This is inconclusive about test coverage and
  no action is needed, but indicates an opportunity for cargo-mutants to either
  generate better mutants, or at least not generate unviable mutants.

* **timeout** — The mutation caused the test suite to run for a long time, until it was eventually killed. You might want to investigate the cause and potentially mark the function to be skipped.

By default only missed mutants and timeouts are printed because they're the most actionable. Others can be shown with the `-v` and `-V` options.

Your goal is to get well-written and well-tested code, and you get to decide (with your co-authors) just what kind of structure and testing you prefer. Cargo-mutants provides information to help you get there, but does not dictate what you should do.

When faced with a missed mutant there are many ways to "cheat" and make the mutant no longer missed, without really improving  the program. For example, you could:

* inline the function into its callers, so that cargo-mutants no longer sees it as a function that can be mutated
* mark it as skipped
* add a unit test that checks the return value of the specific function
* delete the function and the code that calls it

These might be appropriate choices but they also might not move the program in the right direction.

One good question is: why didn't an existing test catch this? Some programs, like cargo-mutants itself, are designed to be tested primarily on their public interface, either an API, a command-line interface or a network API. Ask which part of the public interface's behavior should have failed if this function was mutated. Why didn't a test already catch that? Could you extend an existing test to check it?

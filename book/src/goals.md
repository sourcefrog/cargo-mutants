# Goals

**The goal of cargo-mutants is to be _easy_ to run on any Rust source tree, and
to tell you something _interesting_ about areas where bugs might be lurking or
the tests might be insufficient.**

The detailed goals in this section are intended to generally guide development
priorities and tradeoffs. For example, the goal of _ease_ means that we will
generally prefer to automatically infer reasonable default behavior rather than
requiring the user to configure anything at first. The goal of being
_interesting_ means that we will generally only enable by default features that
seem reasonably likely to say something important about test quality to at least
some users.

## Ease

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
  tests, so it's relatively OK if it takes a while.

  (There is currently very little overhead beyond the cost to do an incremental
  build and run the tests for each mutant, but that can still take a while for
  large trees that produce many mutants especially if their test suite takes a
  while.)

- cargo-mutants should run correctly on any Rust source trees that are built and
  tested by Cargo, that will build and run their tests in a copy of the tree,
  and that have hermetic tests.

- cargo-mutants shouldn't crash or hang, even if it generates mutants that cause
  the software under test to crash or hang.

- The results should be reproducible, assuming the build and test suite is
  deterministic.

- cargo-mutants should avoid generating unviable mutants that don't compile,
  because that wastes time. However, when it's uncertain whether the mutant will
  build, it's worth trying things that _might_ find interesting results even if
  they might fail to build.  (It does currently generate _some_ unviable
  mutants, but typically not too many, and they don't have a large effect on
  runtime in most trees.)

- Realistically, cargo-mutants may generate some mutants that aren't caught by
  tests but also aren't interesting, or aren't feasible to test. In those cases
  it should be easy to permanently dismiss them (e.g. by adding a
  `#[mutants::skip]` attribute or a config file.)

## Interestingness

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
  it's not interesting that the test suite doesn't catch the change.

- For trees that are thoroughly tested, you can use `cargo mutants` in CI to
  check that they remain so.

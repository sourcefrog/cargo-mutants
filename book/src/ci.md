# Continuous integration

You might want to use cargo-mutants in your continuous integration (CI) system, to ensure that no uncaught mutants are merged into your codebase.

There are at least two complementary ways to use cargo-mutants in CI:

1. [Check for mutants produced in the code changed in a pull request](pr-diff.md). This is typically _much_ faster than testing all mutants, and is a good way to ensure  that newly merged code is well tested, and to facilitate conversations about how to test the PR.

2. Checking that all mutants are caught, on PRs or on the development branch.

## Recommendations for CI

* Use the [`--in-place`](in-place.md) option to avoid copying the tree.

## Installing into CI

The recommended way to install cargo-mutants is using [install-action](https://github.com/taiki-e/install-action), which will fetch a binary from cargo-mutants most recent GitHub release, which is faster than building from source. You could alternatively use [baptiste0928/cargo-install](https://github.com/baptiste0928/cargo-install) which will build it from source in your worker and cache the result.

## Example workflow

Here is an example of a GitHub Actions workflow that runs mutation tests and uploads the results as an artifact. This will fail if it finds any uncaught mutants.

```yml
name: cargo-mutants

env:
  CARGO_TERM_COLOR: always

on:
  push:
    branches:
      - main
  pull_request:
    # Only test PR if it changes something that's likely to affect the results, because
    # mutant tests can take a long time. Adjust these paths to suit your project.
    paths:
      - ".cargo/**"
      - ".github/workflows/mutants.yml"
      - "Cargo.*"
      - "src/**"
      - "testdata/**"
      - "tests/**"

jobs:
  cargo-mutants:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - uses: taiki-e/install-action@v2
        name: Install cargo-mutants using install-action
        with:
          tool: cargo-mutants
      - name: Run mutant tests
        run: cargo mutants -vV --in-place
      - name: Archive results
        uses: actions/upload-artifact@v4
        if: always()
        with:
          name: mutants-out
          path: mutants.out
```

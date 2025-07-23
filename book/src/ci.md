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

The recommended way to install cargo-mutants is using [install-action](https://github.com/taiki-e/install-action), which will fetch a binary from cargo-mutants most recent GitHub release, which is faster than building from source. You could alternatively use [baptiste0928/cargo-install](https://github.com/baptiste0928/cargo-install) which will build it from source in your worker and cache the result.

```yml
{{#include ../../examples/workflows/basic.yml}}
```

The workflow used by cargo-mutants on itself can be seen at
<https://github.com/sourcefrog/cargo-mutants/blob/main/.github/workflows/tests.yml>, but this is different from what you will typically want to use, because it runs cargo-mutants from HEAD.

## Annotations

cargo-mutants will emit GitHub Actions structured annotations when it detects that it's running within an action. (Specifically, when `$GITHUB_ACTION` is set.)

This behavior can be forced on with the `--annotations=github` option, or off with `--annotations=none`.

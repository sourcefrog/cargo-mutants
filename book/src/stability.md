# Stability

## Reproducibility within a single version

The results of running `cargo mutants` should be deterministic and reproducible assuming that the build and test process for the code under test is also deterministic. Any nondeterminism is a bug.

By default, the order in which mutants are tested is randomized. If the tests are hermetic, this should make no difference other than the order in which the output is presented. This can be disabled with `--no-shuffle`.

If multiple parallel jobs are run, the results should be the same as running the same number of serial jobs, except for the order of the output.

## Reproducibility across versions

cargo-mutants behavior may change between versions, although we will attempt to minimize disruption and to document any changes in the [changelog](changelog.md).

In particular the following changes can be expected:

- Addition of new mutation patterns, so that later versions generate new mutants.
- Removal or changes of existing mutation patterns if they turn out to generate too many unviable mutants or too few interesting mutants.
- Changes to the built-in heuristics controlling what code is skipped or mutated. For example, an earlier version failed to skip functions marked with `#![cfg(test)]` and this was fixed in a later version.
- Addition of new information to the JSON output files. Removal of existing files or fields will be avoided where possible.
- Changes to the presentation of mutant names in the console and in JSON.
- Changes to console output and progress.

As a result of all these, a tree that passes all mutants in one version may fail some in a later version, and vice versa.

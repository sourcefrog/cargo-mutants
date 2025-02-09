# Testing in place

By default, cargo-mutants copies your code to a temporary directory, where it applies mutations and then runs tests there.

With the `--in-place` option, it will instead mutate and test your code in the original source directory.

`--in-place` is currently incompatible with the [`--jobs` option](parallelism.md), because running multiple jobs requires making multiple copies of the tree.

## Cautions

If you use `--in-place` then you shouldn't edit the code, commit, or run your own tests while tests are running, because cargo-mutants will be modifying the code at the same time.

cargo-mutants will try to restore the code to its original state after testing each mutant, but it's possible that it might fail to do so if it's interrupted or panics.

When using `--in-place`, either:

1. Use a dedicated disposable checkout, or
2. Review all diffs carefully before committing

You can detect mutations by searching for this marker:

    /* ~ changed by cargo-mutants ~ */

## Why test in place?

Some situations where `--in-place` might be useful are:

* You're running cargo-mutants in CI with a source checkout that exists solely for testing, so it would be a waste of time and space to copy it.
* You've previously built the tree into `target` and want to avoid rebuilding it: the Rust toolchain currently doesn't reuse build products after cargo-mutants copies the tree, but it will reuse them with `--in-place`.
* The source tree is extremely large, and making a copy would use too much disk space, or take time that you don't want to spend. (In most cases copying the tree takes negligible time compared to running the tests, but if it contains many binary assets it might be significant.)
* Your tree only works when built at a specific filesystem path, or it refers to other directories that cargo-mutants doesn't understand how to copy.
* You're investigating or debugging a problem where the tests don't pass in a copy of the tree. (Please report this as a bug if you can describe how to reproduce it.)

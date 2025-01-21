# Cautions

cargo-mutants is generally as safe to run as `cargo test`, but there are two important safety considerations:

## Test side effects

cargo-mutants introduces machine-generated modifications that simulate bugs.

If your tests have external side effects (like file operations), these modifications could cause unintended consequences. For example, while cargo-mutants won't add new file deletion calls, it might modify existing deletion paths in your code. A bug in such code could potentially delete unintended directories.

Most test suites are designed to limit side effects to temporary test resources, even when buggy. However, to minimize risk:

1. Maintain frequent remote backups
1. Run in an isolated environment (container, CI, or VM)
1. Avoid using production credentials in tests

## Source tree modifications

By default, cargo-mutants works on a copy of your source tree and only writes to a `mutants.out` directory.

However, when using [`--in-place`](in-place.md), it modifies your original source directory. While these changes are normally reverted after testing, sudden interruptions may leave mutations in place.

When using `--in-place`, either:

1. Use a dedicated disposable checkout, or
2. Review all diffs carefully before committing

You can detect mutations by searching for this marker:

    /* ~ changed by cargo-mutants ~ */

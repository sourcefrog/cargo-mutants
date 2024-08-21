# Testing code changed in a diff

If you're working on a large project or one with a long test suite, you may not want to test the entire codebase every time you make a change. You can use `cargo-mutants --in-diff` to test only mutants generated from recently changed code.

The `--in-diff DIFF_FILE` option tests only mutants that overlap with regions changed in the diff.

The diff is expected to either have a prefix of `b/` on the new filename, which is the format produced by `git diff`, or no prefix.

Some ways you could use `--in-diff`:

1. Before submitting code, check your uncommitted changes with `git diff`.
2. In CI, or locally, check the diff between the current branch and the base branch of the pull request.

Changes to non-Rust files, or files from which no mutants are produced, are ignored.

`--in-diff` is applied on the output of other filters including `--package` and `--regex`. For example, `cargo mutants --in-diff --package foo` will only test mutants in the `foo` package that overlap with the diff.

## Caution

`--in-diff` makes tests faster by covering the mutants that are most likely to be missed in the changed code. However, it's certainly possible that edits in one region cause code in a different region or a different file to no longer be well tested. Incremental tests are helpful for giving faster feedback, but they're not a substitute for a full test run.

The diff is only matched against the code under test, not the test code. So, a diff that only deletes or changes test code won't cause any mutants to run, even though it may have a very material effect on test coverage.

## Example

In this diff, we've added a new function `two` to `src/lib.rs`, and the existing code is unaltered. With `--in-diff`, `cargo-mutants` will only test mutants that affect the function `two`.

```diff

```diff
--- a/src/lib.rs    2023-11-12 13:05:25.774658230 -0800
+++ b/src/lib.rs    2023-11-12 12:54:04.373806696 -0800
@@ -2,6 +2,10 @@
     "one".to_owned()
 }

+pub fn two() -> String {
+    format!("{}", 2)
+}
+
 #[cfg(test)]
 mod test_super {
     use super::*;
@@ -10,4 +14,9 @@
     fn test_one() {
         assert_eq!(one(), "one");
     }
+
+    #[test]
+    fn test_two() {
+        assert_eq!(two(), "2");
+    }
 }
```

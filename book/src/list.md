# Listing generated mutants

`--list`: Show what mutants could be generated, without running them.

`--diff`: With `--list`, also include a diff of the source change for each mutant.

`--json`: With `--list`, show the list in json for easier processing by other programs.
(The same format is written to `mutants.out/mutants.json` when running tests.)

`--check`: Run `cargo check` on all generated mutants to find out which ones are viable, but don't actually run the tests. (This is primarily useful when debugging cargo-mutants.)

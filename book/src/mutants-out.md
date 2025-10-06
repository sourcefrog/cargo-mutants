# The `mutants.out` directory

A `mutants.out` directory is created in the original source directory. You can put the output directory elsewhere with the `--output` option
or using `CARGO_MUTANTS_OUTPUT` environment variable or via `output` directive in the config file.

On each run, any existing `mutants.out` is renamed to `mutants.out.old`, and any
existing `mutants.out.old` is deleted.

The output directory contains:

* A `lock.json`, on which an [fs2 lock](https://docs.rs/fs2) is held while
  cargo-mutants is running, to avoid two tasks trying to write to the same
  directory at the same time. `lock.json` contains the start time, cargo-mutants
  version, username, and hostname. `lock.json` is left in `mutants.out` when the
  run completes, but the lock on it is released.

* A `mutants.json` file describing all the generated mutants.
  This file is completely written before testing begins.

* An `outcomes.json` file describing the results of all tests,
  summary counts of each outcome, and the cargo-mutants version.

* A `diff/` directory, containing a diff file for each mutation, relative to the unmutated baseline.
  `mutants.json` includes for each mutant the name of the diff file.

* A `logs/` directory, with one log file for each mutation plus the baseline
  unmutated case. The log contains the diff of the mutation plus the output from
  cargo. `outcomes.json` includes for each mutant the name of the log file.

* `caught.txt`, `missed.txt`, `timeout.txt`, `unviable.txt`, each listing mutants with the corresponding outcome.

* `previously_caught.txt` accumulates a list of mutants caught in previous runs with [`--iterate`](iterate.md).

The contents of the directory and the format of these files is subject to change in future versions.

These files are incrementally updated while cargo-mutants runs, so other programs can read them to follow progress.

There is generally no reason to include this directory in version control, so it is recommended that you add `/mutants.out*` to your `.gitignore` file or equivalent. This will exclude both `mutants.out` and `mutants.out.old`.

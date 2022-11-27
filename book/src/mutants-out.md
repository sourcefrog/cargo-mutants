# mutants.out

A `mutants.out` directory is created in the source directory, or whichever directory you specify with `--output`. It contains:

* A `lock.json`, on which an [fs2 lock](https://docs.rs/fs2) is held while
  cargo-mutants is running, to avoid two tasks trying to write to the same
  directory at the same time. `lock.json` contains the start time, cargo-mutants
  version, username, and hostname. `lock.json` is left in `mutants.out` when the
  run completes, but the lock on it is released.

* A `mutants.json` file describing all the generated mutants.
  This file is completely written before testing begins.

* An `outcomes.json` file describing the results of all tests,
  and summary counts of each outcome.

* A `logs/` directory, with one log file for each mutation plus the baseline
  unmutated case. The log contains the diff of the mutation plus the output from
  cargo. `outcomes.json` includes for each mutant the name of the log file.

* `caught.txt`, `missed.txt`, `timeout.txt`, `unviable.txt`, each listing mutants with the corresponding outcome.

The contents of the directory and the format of these files is subject to change in future versions.

These files are incrementally updated while cargo-mutants runs, so other programs can read them to follow progress.

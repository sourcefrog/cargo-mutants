# cargo-mutants changelog

## Unreleased

- Added: A new `mutants.out/debug.log` with internal debugging information.

## 0.2.10

Released 2022-08-07

cargo-mutants 0.2.10 comes with improved docs, and the new `-C` option can be used to pass options like `--release` or `--all-features` to `cargo`.

- Added: `--cargo-arg` (or `-C` for short) allows passing arguments to cargo commands (check, build, and test), for example to set `--release` or `--features`.

- Improved: Works properly if run from a subdirectory of a crate, or if `-d` points to a
  subdirectory of a crate.

- Improved: Various docs.

- Improved: Relative dependencies within the source tree are left as relative paths, and will be built within the scratch directory. Relative dependencies outside the source tree are still rewritten as absolute paths.

## 0.2.9

Released 2022-07-30

- Faster: `cargo mutants` no longer runs `cargo check` before building, in cases where the build products are wanted or tests will be run. This saves a significant amount of work in build phases; in some trees `cargo mutants` is now 30% faster. (In trees where most of the time is spent running tests the effect will be less.)

- Fixed: Open log files in append mode to fix messages from other processes
  occasionally being partly overwritten.

- Improved: `cargo mutants` should now give useful results in packages that use `#![deny(unused)]` or other mechanisms to reject warnings.  Mutated functions often ignore some parameters, which would previously be rejected by this configuration without proving anything interesting about test coverage. Now, `--cap-lints=allow` is passed in `RUSTFLAGS` while building mutants, so that they're not falsely rejected and the tests can be exercised.

- Improved: The build dir name includes the root package name.

- Improved: The progress bar shows more information.

- Improved: The final message shows how many mutants were tested and how long it took.

## 0.2.8

Released 2022-07-18

- New: Summarize the overall number of mutants generated, caught, missed, etc, at the end.

- Fixed: Works properly with crates that have relative `path` dependencies in `Cargo.toml` or `.cargo/config.toml`, by rewriting them to absolute paths in the scratch directory.

## 0.2.7

Released 2022-07-11

- New: You can skip functions by adding `#[cfg_attr(test, mutants::skip)`, in which case the `mutants` crate can be only a `dev-dependency`.

- Improved: Don't generate pointless mutations of functions with an empty body (ignoring comments.)

- Improved: Remove extra whitespace from the display of function names and return types: the new formatting is closer to the spacing used in idiomatic Rust.

- Improved: Show the last line of compiler/test output while running builds, so that it's more clear where time is being spent.

- Docs: Instructions on how to check for missed mutants from CI.

## 0.2.6

Released 2022-04-17

- Improved: Find source files by looking at `cargo metadata` output, rather than
  assuming they're in `src/**/*.rs`. This makes `cargo mutants` work properly
  on trees where it previously failed to find the source.

- New `--version` option.

- New: Write a `lock.json` into the `mutants.out` directory including the start
  timestamp, cargo-mutants version, hostname and username. Take a lock on this
  file while `cargo mutants` is running, so that it doesn't crash or get
  confused if two tasks try to write to the same directory at the same time.

- New: Restored a `--list-files` option.

- Changed: Error if no mutants are generated, which probably indicates a bug
  or configuration error(?)

## 0.2.5

Released 2022-04-14

- New `--file` command line option to mutate only functions in source files
  matching a glob.

- Improved: Don't attempt to mutate functions called `new` or implementations of
  `Default`. cargo-mutants can not yet generate good mutations for these so they
  are generally false positives.

- Improved: Better display of `<impl Foo for Bar>::foo` and similar type paths.

- New: `--output` directory to write `mutants.out` somewhere other than the
  source directory.

## 0.2.4

Released 2022-03-26

- Fix: Ignore errors setting file mtimes during copies, which can cause failures on
  Windows if some files are readonly.

- Fix: Log file names now include only the source file relative path, the line
  number, and a counter, so they are shorter, and shouldn't cause problems on
  filesystems with length limits.

- Change: version-control directories like `.git` are not copied with the source
  tree: they should have no effect on the build, so copying them is just a
  waste.

- Changed/improved json logs in `mutants.out`:

  - Show durations as fractional seconds.

  - Outcomes include a "summary" field.

## 0.2.3

Released 2022-03-23

- Switch from Indicatif to [Nutmeg](https://github.com/sourcefrog/nutmeg) to
  draw progress bars and output. This fixes a bug where terminal output
  line-wraps badly, and adds a projection for the total estimated time to
  completion.

- Change: Mutants are now tested in random order by default, so that repeated
  runs are more likely to surface interesting new findings early, rather
  than repeating previous results. The previous behavior of testing mutants
  in the deterministic order they're encountered in the tree can be restored
  with `--no-shuffle`.

## 0.2.2

Released 2022-02-16

- The progress bar now shows which mutant is being tested out of how many total.

- The automatic timeout is now set to the minimum of 20 seconds, or 5x the time
  of the tests in a baseline tree, to reduce the incidence of false timeouts on
  machines with variable throughput.

- Ctrl-c (or `SIGINT`) interrupts the program during copying the tree.
  Previously it was not handled until the copy was complete.

- New `--no-copy-target` option.

## 0.2.1

Released 2022-02-10

- Arguments to `cargo test` can be passed on the command line after `--`. This
  allows, for example, skipping doctests or setting the number of test threads.
  <https://github.com/sourcefrog/cargo-mutants/issues/15>

## 0.2.0

Released 2022-02-06

- A new `--timeout SECS` option to limit the runtime of any `cargo test`
  invocation, so that mutations that cause tests to hang don't cause
  `cargo mutants` to hang.

  A default timeout is set based on the time to run tests in an unmutated tree.
  There is no timeout by default on the unmutated tree.

  On Unix, the `cargo` subprocesses run in a new process group. As a consequence
  ctrl-c is explicitly caught and propagated to the child processes.

- Show a progress bar while looking for mutation opportunities, and show the
  total number found.

- Show how many mutation opportunities were found, before testing begins.

- New `--shuffle` option tests mutants in random order.

- By default, the output now only lists mutants that were missed or that timed
  out. Mutants that were caught, and mutants that did not build, can be printed
  with `--caught` and `--unviable` respectively.

## 0.1.0

Released 2021-11-30

- Logs and other information are written into `mutants.out` in the source
  directory, rather than `target/mutants`.

- New `--all-logs` option prints all Cargo output to stdout, which is verbose
  but useful for example in CI, by making all the output directly available in
  captured stdout.

- The output distinguishes check or build failures (probably due to an unviable
  mutant) from test failures (probably due to lacking coverage.)

- A new file `mutants.out/mutants.json` lists all the generated mutants.

- Show function return types in some places, to make it easier to understand
  whether the mutants were useful or viable.

- Run `cargo check --tests` and `cargo build --tests` in the source directory to
  freshen the build and download any dependencies, before copying it to a
  scratch directory.

- New `--check` option runs `cargo check` on generated mutants to see if they
  are viable, without actually running the tests. This is useful in tuning
  cargo-mutants to generate better mutants.

- New `--no-times` output hides times (and tree sizes) from stdout, mostly to
  make the output deterministic and easier to match in tests.

- Mutate methods too!

## 0.0.4

Released 2021-11-10

- Fixed `cargo install cargo-mutants` (sometimes?) failing due to the `derive`
  feature not getting set on the `serde` dependency.

- Show progress while copying the tree.

- Respect the `$CARGO` environment variable so that the same toolchain is used
  to run tests as was used to invoke `cargo mutants`. Concretely,
  `cargo +nightly mutants` should work correctly.

## 0.0.3

Released 2021-11-06

- Skip functions or modules marked `#[test]`, `#[cfg(test)]` or
  `#[mutants::skip]`.

- Early steps towards type-guided mutations:

  - Generate mutations of `true` and `false` for functions that return `bool`
  - Empty and arbitrary strings for functions returning `String`.
  - Return `Ok(Default::default())` for functions that return `Result<_, _>`.

- Rename `--list-mutants` to just `--list`.

- New `--list --json`.

- Colored output makes test names and mutations easier to read (for me at
  least.)

- Return distinct exit codes for different situations including that uncaught
  mutations were found.

## 0.0.2

- Functions that should not be mutated can be marked with `#[mutants::skip]`
  from the [`mutants`](https://crates.io/crates/mutants) helper crate.

## 0.0.1

First release.

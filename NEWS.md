# cargo-mutants changelog

## Unreleased

- New: Colored output can be enabled in CI or other noninteractive situations by passing `--colors=always`, or setting `CARGO_TERM_COLOR=always`, or `CLICOLOR_FORCE=1`. Colors can similarly be forced off with `--colors=never`, `CARGO_TERM_COLOR=never`, or `NO_COLOR=1`.

## 24.1.2

- New: `--in-place` option tests mutations in the original source tree, without copying the tree. This is faster and uses less disk space, but it's incompatible with `--jobs`, and you must be careful not to edit or commit the source tree while tests are running.

## 24.1.1

- New: Mutate `+, -, *, /, %, &, ^, |, <<, >>` binary ops, and their corresponding assignment ops like `+=`.

- New: `--baseline=skip` option to skip running tests in an unmutated tree, when they're already been checked externally.

- Changed: Stop generating mutations of `||` and `&&` to `!=` and `||`, because it seems to raise too many low-value false positives that may be hard to test.

- Fixed: Colors in command-line help and error messages.

## 24.1.0

- New! `cargo mutants --test-tool nextest`, or `test_tool = "nextest"` in `.cargo/mutants.toml` runs tests under [Nextest](https://nexte.st/). Some trees have tests that only work under Nextest, and this allows them to be tested. In other cases Nextest may be significantly faster, because it will exit soon after the first test failure.

- Fixed: Fixed spurious "Patch input contains repeated filenames" error when `--in-diff` is given a patch that deletes multiple files.

## 23.12.2

- New: A `--shard k/n` allows you to split the work across n independent parallel `cargo mutants` invocations running on separate machines to get a faster overall solution on large suites. You, or your CI system, are responsible for launching all the shards and checking whether any of them failed.

- Improved: Better documentation about `-j`, with stronger recommendations not to set it too high.

- New: Binary releases on GitHub through cargo-dist.

## 23.12.1

- Improved progress bars and console output, including putting the outcome of each mutant on the left, and the overall progress bar at the bottom. Improved display of estimated remaining time, and other times.

- Fixed: Correctly traverse `mod` statements within package top source files that are not named `lib.rs` or `main.rs`, by following the `path` setting of each target within the manifest.

- Improved: Don't generate function mutants that have the same AST as the code they're replacing.

## 23.12.0

An exciting step forward: cargo-mutants can now generate mutations smaller than a whole function. To start with, several binary operators are mutated.

- New: Mutate `==` to `!=` and vice versa.

- New: Mutate `&&` to `||` and vice versa, and mutate both of them to `==` and `!=`.

- New: Mutate `<`, `<=`, `>`, `>=`.

- Changed: If no mutants are generated then `cargo mutants` now exits successfully, showing a warning. (Previously it would exit with an error.) This works better with `--in-diff` in CI, where it's normal that some changes may not have any mutants.

- Changed: Include column numbers in text listings of mutants and output to disambiguate smaller-than-function mutants, for example if there are several operators that can be changed on one line. This also applies to the names used for regex matching, so may break some regexps that match the entire line (sorry). The new option `--line-col=false` turns them both off in `--list` output.

- Changed: In the mutants.json format, replaced the `function`, `line`, and `return_type` fields with a `function` submessage (including the name and return type) and a `span` indicating the entire replaced region, to better handle smaller-than-function mutants. Also, the `function` includes the line-column span of the entire function.

## 23.11.2

- Changed: If `--file` or `--exclude` are set on the command line, then they replace the corresponding config file options. Similarly, if `--re` is given then the `examine_re` config key is ignored, and if `--exclude-re` is given then `exclude_regex` is ignored. (Previously the values were combined.) This makes it easier to use the command line to test files or mutants that are normally not tested.

- Improved: By default, files matching gitignore patterns (including in parent directories, per-user configuration, and `info/exclude`) are excluded from copying to temporary build directories. This should improve performance in some large trees with many files that are not part of the build. This behavior can be turned off with `--gitignore=false`.

- Improved: Run `cargo metadata` with `--no-deps`, so that it doesn't download and compute dependency information, which can save time in some situations.

- Added: Alternative aliases for command line options, so you don't need to remember if it's "regex" or "re": `--regex`, `--examine-re`, `--examine-regex` (all for names to include) and `--exclude-regex`.

- Added: Accept `--manifest-path` as an alternative to `-d`, for consistency with other cargo commands.

## 23.11.1

- New `--in-diff FILE` option tests only mutants that are in the diff from the
  given file. This is useful to avoid testing mutants from code that has not changed,
  either locally or in CI.

## 23.11.0

- Changed: `cargo mutants` now tries to match the behavior of `cargo test` when run within a workspace. If run in a package directory, it tests only that package. If run in a workspace that is not a package (a "virtual workspace"), it tests the configured default packages, or otherwise all packages. This can all be overridden with the `--package` or `--workspace` options.

- New: generate key-value map values from types like `BTreeMap<String, Vec<u8>>`.

- Changed: Send trace messages to stderr rather stdout, in part so that it won't pollute json output.

## 23.10.0

- The baseline test (with no mutants) now tests only the packages in which
  mutants will be generated, subject to any file or regex filters. This
  should both make baseline tests faster, and allow testing workspaces in
  which some packages have non-hermetic tests.

## 23.9.1

- Mutate the known collection types `BinaryHeap`, `BTreeSet`, `HashSet`,
  `LinkedList`, and `VecDeque` to generate empty and one-element collections
  using `T::new()` and `T::from_iter(..)`.

- Mutate known container types like `Arc`, `Box`, `Cell`, `Mutex`, `Rc`,
  `RefCell` into `T::new(a)`.

- Mutate unknown types that look like containers or collections `T<A>` or
  `T<'a, A>'` and try to construct them from an `A` with `T::from_iter`,
  `T::new`, and `T::from`.

- Minimum Rust version updated to 1.70.

- Mutate `Cow<'_, T>` into `Owned` and `Borrowed` variants.

- Mutate functions returning `&[T]` and `&mut [T]` to return leaked vecs
  of values.

- Mutate `(A, B, C, ...)` into the product of all replacements for
  `a, b, c, ...`

- The combination of options `--list --diff --json` is now supported, and emits
  a `diff` key in the JSON.

- Mutate `-> impl Iterator<Item = A>` to produce empty and one-element iterators
  of the item type.

## 23.9.0

- Fixed a bug causing an assertion failure when cargo-mutants was run from a
  subdirectory of a workspace. Thanks to Adam Chalmers!

- Generate `HttpResponse::Ok().finish()` as a mutation of an Actix `HttpResponse`.

## 23.6.0

- Generate `Box::leak(Box::new(...))` as a mutation of functions returning
  `&mut`.

- Add a concept of mutant "genre", which is included in the json listing of
  mutants. The only genre today is `FnValue`, in which a function body is
  replaced by a value. This will in future allow filtering by genre.

- Recurse into return types, so that for example `Result<bool>` can generate
  `Ok(true)` and `Ok(false)`, and `Some<T>` generates `None` and every generated
  value of `T`. Similarly for `Box<T>`, `Vec<T>`, `Rc<T>`, `Arc<T>`.

- Generate specific values for integers: `[0, 1]` for unsigned integers,
  `[0, 1, -1]` for signed integers; `[1]` for NonZero unsigned integers and
  `[1, -1]` for NonZero signed integers.

- Generate specific values for floats: `[0.0, 1.0, -1.0]`.

- Generate (fixed-length) array values, like `[0; 256], [1; 256]` using every
  recursively generated value for the element type.

## 23.5.0

_"Pickled crab"_

Released 2023-05-27

- `cargo mutants` can now successfully test packages that transitively depend on
  a different version of themselves, such as `itertools`. Previously,
  cargo-mutants used the cargo `--package` option, which is ambiguous in this
  case, and now it uses `--manifest-path` instead.

- Mutate functions returning `&'_ str` (whether a lifetime is named or not) to
  return `"xyzzy"` and `""`.

- Switch to CalVer numbering.

## 1.2.3

Released 2023-05-05

- Mutate functions returning `String` to `String::new()` rather than `"".into()`: same
  result but a bit more idiomatic.

- New `--leak-dirs` option, for debugging cargo-mutants.

- Update to [syn 2.0](https://github.com/dtolnay/syn/releases/tag/2.0.0), adding support for new Rust syntax.

- Minimum supported Rust version increased to 1.65 due to changes in dependencies.

- New `--error` option, to cause functions returning `Result` to be mutated to return the
  specified error.

- New `--no-config` option, to disable reading `.cargo/mutants.toml`.

## 1.2.2

Released 2023-04-01

- Don't mutate `unsafe` fns.

- Don't mutate functions that never return (i.e. `-> !`).

- Minimum supported Rust version increased to 1.64 due to changes in dependencies.

- Some command-line options can now also be configured through environment variables:
  `CARGO_MUTANTS_JOBS`, `CARGO_MUTANTS_TRACE_LEVEL`.

- New command line option `--minimum-test-timeout` and config file variable `minimum_test_timeout`
  join existing environment variable `CARGO_MUTANTS_MINIMUM_TEST_TIMEOUT`, to allow
  boosting the minimum, especially for test environments with poor or uneven throughput.

- Changed: Renamed fields in `outcomes.json` from `cargo_result` to `process_status` and from `command` to `argv`.

- Warn if no mutants were generated or if all mutants were unviable.

## 1.2.1

Released 2023-01-05

- Converted most of the docs to a book available at <https://mutants.rs/>.

- Fixed: Correctly find submodules that don't use mmod.rs`naming, e.g. when
descending from`src/foo.rs`to`src/foo/bar.rs`. Also handle module names that
are raw identifiers using`r#`. (Thanks to @kpreid for the report.)

## 1.2.0

_Thankful mutants!_

- Fixed: Files that are excluded by filters are also excluded from `--list-files`.

- Fixed: `--exclude-re` and `--re` can match against the return type as shown in
  `--list`.

- New: A `.cargo/mutants.toml` file can be used to configure standard filters
  and cargo args for a project.

## 1.1.1

Released 2022-10-31

_Spooky mutants!_

- Fixed support for the Mold linker, or for other options passed via `RUSTFLAGS` or `CARGO_ENCODED_RUSTFLAGS`. (See the instructions in README.md).

- Source trees are walked by following `mod` statements rather than globbing the directory. This is more correct if there are files that are not referenced by `mod` statements. Once attributes on modules are stable in Rust (<https://github.com/rust-lang/rust/issues/54727>) this opens a path to skip mods using attributes.

## 1.1.0

Released 2022-10-30

_Fearless concurrency!_

- cargo-mutants can now run multiple cargo build and test tasks in parallel, to make better use of machine resources and find mutants faster, controlled by `--jobs`.

- The minimum Rust version to build cargo-mutants is now 1.63.0. It can still be used to test code under older toolchains.

## 1.0.3

Released 2022-09-29

- cargo-mutants is now finds no uncaught mutants in itself! Various tests were added and improved, particularly around handling timeouts.

- New: `--re` and `--exclude-re` options to filter by mutant name, including the path. The regexps match against the strings printed by `--list`.

## 1.0.2

Released 2022-09-24

- New: `cargo mutants --completions SHELL` to generate shell completions using `clap_complete`.

- Changed: `cargo-mutants` no longer builds in the source directory, and no longer copies the `target/` directory to the scratch directory. Since `cargo-mutants` now sets `RUSTFLAGS` to avoid false failures from warnings, it is unlikely to match the existing build products in the source directory `target/`, and in fact building there is just likely to cause rebuilds in the source. The behavior now is as if `--no-copy-target` was always passed. That option is still accepted, but it has no effect.

- Changed: `cargo-mutants` finds all possible mutations before doing the baseline test, so that you can see earlier how many there will be.

- New: Set `INSTA_UPDATE=no` so that tests that use the [Insta](https://insta.rs/) library don't write updates back into the source directory, and so don't falsely pass.

## 1.0.1

Released 2022-09-12

- Fixed: Don't try to mutate functions within test targets, e.g. within `tests/**/*.rs`.

- New: `missed.txt`, `caught.txt`, `timeout.txt` and `unviable.txt` files are written in to the output directory to make results easier to review later.

- New: `--output` creates the specified directory if it does not exist.

- Internal: Switched from Argh to Clap for command-line parsing. There may be some small changes in CLI behavior and help formatting.

## 1.0.0

Released 2022-08-21

A 1.0 release to celebrate that with the addition of workspace handling, cargo-mutants gives useful results on many Rust projects.

- New: Supports workspaces containing multiple packages. Mutants are generated for all relevant targets in all packages, and mutants are subject to the tests of their own package.  `cargo mutants --list-files --json` and `cargo mutants --list --json` now includes package names for each file or mutant.

- Improved: Generate mutations in `cdylib`, `rlib`, and ever other `*lib` target. For example, this correctly exercises Wasm projects.

- Improved: Write `mutants.out/outcomes.json` after the source-tree build and baseline tests so that it can be observed earlier on.

- Improved: `mutants.out/outcomes.json` includes the commands run.

## 0.2.11

Released 2022-08-20

- New `--exclude` command line option to exclude source files from mutants generation, matching a glob.

- New: `CARGO_MUTANTS_MINIMUM_TEST_TIMEOUT` sets a minimum timeout for cargo tests, in seconds. This can be used to allow more time on slow CI builders. If unset the default is still 20s.

- Added: A new `mutants.out/debug.log` with internal debugging information.

- Improved: The time for check, build, and test is now shown separately in progress bars and output, to give a better indication of which is taking more time in the tree under test. Also, times are show in seconds with one decimal place, and they are styled more consistently.

- Improved: More consistent use of 'unviable' and other terms for outcomes in the UI.

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

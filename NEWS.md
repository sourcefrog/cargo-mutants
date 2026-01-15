# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

- Diffs are now included in the JSON output in `mutants.json` and shown by `--list --json`.

## [26.1.2](https://github.com/sourcefrog/cargo-mutants/compare/cargo-mutants-v26.1.1...cargo-mutants-v26.1.2) - 2026-01-15

### Changed

- Revert "ci: Don't need to run mdbook-linkcheck again during deploy"
- Simpler changelog header
- Shallow fetch in more CI workflows
- Don't need to run mdbook-linkcheck again during deploy
- Fix git ref in book deploy job
- Merge the two book workflows

## 26.1.1 - 2026-01-15

### Other

- Switch to using release-plz for release automation.
 
# cargo-mutants changelog

## 26.1.0 2026-01-15

- New: Added a `name` field in the JSON representation of mutants. Thanks to @0xLoopTheory.

- Changed: Minimum supported Rust version increased to 1.88.

- Fixed: Fix build on musl by disabling, for now, reflink copies.

- Fixed: Don't error on diffs that contain only git file moves, by moving to Flickzeug. Thanks to @eirnym.

## 26.0.0 2025-12-07

- Changed: The default is now *not* to shuffle mutants: they run in the deterministic order they are generated in the source tree. This should give somewhat better locality of reference due to consecutively testing changes in each package or module. The previous behavior can be restored with `--shuffle`.

- Changed: `mutants.json` is now written from the set of mutations that will actually be tested after applying all filters, excluding mutants that have already been tested in previous iterations (if `--iterate` is used). Previously it listed all generated mutants, even if they would not be tested.

- Changed: Removed the custom Debug representation of Mutant because it was not particularly useful.

- Changed: The JSON output from listing or running includes a mutant `name` field that is a text name like those in the various `*.txt` logs.

- New: `cargo mutants --diff` adds unified diffs to each mutant when it is listed.

## 25.3.1 2025-08-10

- Fixed: cargo-mutants' own tests were failing on nightly due to a change in the format of messages emitted by tests.

## 25.3.0 2025-08-10

- New: A specific clearer error if a valid non-empty diff changes no Rust source files, and so matches no mutants. Thanks to @brunoerg.

- New: cargo-mutants can emit GitHub Actions structured annotations for missed mutants, which appear as warnings outside of the log text. This behavior is on by default when the `GITHUB_ACTION` environment variable is set, can be forced on with `--annotations=github` and forced off with `--annotations=none`.

## 25.2.2 2025-07-18

- Changed: The mutant name of "replace match guard" mutations now includes the original match guard, for example `replace match guard path.path.is_ident("str") with true in type_replacements`. Similarly, the "delete match arm" mutation includes the pattern of the arm, for example `delete match arm BinOp::BitOr(_) in ...`.

- Internal: Automatically publish cargo-mutants to crates.io from GitHub Actions.

## 25.2.1 2025-07-10

- Fixed: Updated to `syn` 2.0.104, which understands new Rust syntax including impl trait precise capturing.

## 25.2.0 2025-06-30

- New: `gitignore` config key in `.cargo/mutants.toml` to control whether `.gitignore` patterns are respected when copying source trees, corresponding to `--gitignore`.

- Changed: The mutant name for mutations of `match` statements and guard expressions now includes the enclosing function name, for example `replace match guard with true in find_path_attribute`.

## 25.1.0 2025-06-05

- **Changed**: The `--gitignore` option now defaults to `false`, meaning `.gitignore` patterns are no longer respected when copying source trees by default. The `/target` directory is still excluded by default through explicit filtering. To restore the previous behavior, use `--gitignore=true`.

- New: Mutate `>` to `>=` and `<` to `<=`.

- Changed: Mutate `&T` to `Box::leak(Box::new(...))`, instead of a reference to a value, so that mutants aren't unviable due to returning references to temporary values.

- New: `--copy-target` option allows copying the `/target` directory to build directories. By default, the target directory is excluded to avoid copying large build artifacts, but `--copy-target=true` can be used if tests depend on existing build artifacts.

- New: Feature-related options can now be configured in `.cargo/mutants.toml`: `features`, `all_features`, and `no_default_features`. Command line arguments take precedence over config file settings for boolean options, while features from both sources are combined.

- New: Produce a json schema for the config file with `--emit-schema=config` to support schema-guided editing. The schema has been proposed to SchemaStore so many editors should in future support it automatically.

- New: The config file path can be specified with the `--config` option, overriding the default of `.cargo/mutants.toml`. (The pre-existing `--no-config` option turns it off.)

## 25.0.1 2025-02-08

- New: Additional mutation patterns: delete `match` arms if there is a default arm, and replace `if` guards from match arms with `true` and `false`.

- Changed: Show more type parameters in mutant names, like `impl From<&str> for Foo` rather than `impl From for Foo`.

- Fixed: Support crates that use a non-default Cargo registry. Previously, `cargo metadata` failed with "registry index was not found."

- Improved: Warn if `--jobs` is set higher than 8, which is likely to be too high.

- Improved: Don't warn about expected/harmless exit codes from Nextest.

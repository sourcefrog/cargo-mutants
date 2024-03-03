# Filtering files

Two options (each with short and long names) control which files are mutated:

- `-f GLOB`, `--file GLOB`: Mutate only functions in files matching the glob.

- `-e GLOB`, `--exclude GLOB`: Exclude files that match the glob.

These options may be repeated.

If any `-f` options are given, only source files that match are
considered; otherwise all files are considered. This list is then further
reduced by exclusions.

Globs are treated differently depending on whether they contain a path separator or not.
`/` matches the path separator on both Unix and Windows. `\\` matches the path separator on Windows and is an escape character on Unix.

If the glob contains a path separator then it matches against the path from the root of the source
tree. For example, `src/*/*.rs` will match (and exclude or exclude) all files in subdirectories of `src`. Matches on paths can use `**` to match zero or more directory components: `src/**/*.rs` will match all `.rs` files in `src` and its subdirectories.

If the glob does not contain a path separator, it matches against file and directory names, in any directory. For example, `t*.rs` will match all files whose name start with `t` and ends with `.rs`, in any directory.
in any directory.  `--exclude console` excludes all files within directories called "console", but not files called "console.rs".

Note that the glob must contain `.rs` (or a matching wildcard) to match
source files with that suffix. For example, `-f network` will match
`src/network/mod.rs` but it will _not_ match `src/network.rs`.

Files that are excluded are still parsed (and so must be syntactically
valid), and `mod` statements in them are followed to discover other
source files. So, for example, you can exclude `src/main.rs` but still
test mutants in other files referenced by `mod` statements in `main.rs`.

Since Rust does not currently allow attributes such as `#[mutants::skip]` on `mod` statements or at module scope filtering by filename is the only way to skip an entire module.

The results of filters can be previewed with the `--list-files` and `--list`
options.

Examples:

- `cargo mutants -f visit.rs -f change.rs` -- test mutants only in files
  called `visit.rs` or `change.rs` (in any directory).

- `cargo mutants -e console.rs` -- test mutants in any file except `console.rs`.

- `cargo mutants -f src/db/*.rs` -- test mutants in any file in this directory. This could also be written as `-f src/db`, or (if all the source is in `src`) as `-f db`.

## Configuring filters by filename

Files may also be filtered with the `exclude_globs` and `examine_globs` options in `.cargo/mutants.toml`.

Exclusions in the config file may be particularly useful when there are modules that are
inherently hard to automatically test, and the project has made a decision to accept lower
test coverage for them.

From cargo-mutants 23.11.2 onwards, if the command line options are given then the corresponding config file option is ignored.
This allows you to use the config file to test files that are normally expected to pass, and then
to use the command line to test files that are not yet passing.

For example:

```toml
exclude_globs = ["src/main.rs", "src/cache/*.rs"] # like -e
examine_globs = ["src/important/*.rs"] # like -f: test *only* these files
```

# Filtering files

Two options (each with short and long names) control which files are mutated:

`-f GLOB`, `--file GLOB`: Mutate only functions in files matching
the glob.

`-e GLOB`, `--exclude GLOB`: Exclude files that match the glob.

These options may be repeated.

If any `-f` options are given, only source files that match are
considered; otherwise all files are considered. This list is then further
reduced by exclusions.

If the glob contains `/` (or on Windows, `\`), then it matches against the path from the root of the source
tree. For example, `src/*/*.rs` will exclude all files in subdirectories of `src`.

If the glob does not contain a path separator, it matches against filenames
in any directory.

`/` matches the path separator on both Unix and Windows.

Note that the glob must contain `.rs` (or a matching wildcard) to match
source files with that suffix. For example, `-f network` will match
`src/network/mod.rs` but it will _not_ match `src/network.rs`.

Files that are excluded are still parsed (and so must be syntactically
valid), and `mod` statements in them are followed to discover other
source files. So, for example, you can exclude `src/main.rs` but still
test mutants in other files referenced by `mod` statements in `main.rs`.

The results of filters can be previewed with the `--list-files` and `--list`
options.

Examples:

- `cargo mutants -f visit.rs -f change.rs` -- test mutants only in files
  called `visit.rs` or `change.rs` (in any directory).

- `cargo mutants -e console.rs` -- test mutants in any file except `console.rs`.

- `cargo mutants -f src/db/*.rs` -- test mutants in any file in this directory.

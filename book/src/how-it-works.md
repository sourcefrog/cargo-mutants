# How cargo-mutants works

The basic approach is:

- Build a list of mutations:
  - Run `cargo metadata` to find directories containing Rust source files.
  - Walk all source files and parse each one looking for functions.
  - Skip functions that should not be mutated for any of several reasons:
    because they're tests, because they have a `#[mutants::skip]` attribute,
    etc.
  - For each function, depending on its return type, generate every mutation
    pattern that produces a result of that type.

- Make a copy of the source tree into a scratch directory, excluding
  version-control directories like `.git` and the `/target` directory. The same directory is reused across all the mutations to benefit from incremental builds.
  - After copying the tree, cargo-mutants scans the top-level `Cargo.toml` and any
    `.cargo/config.toml` for relative dependencies. If there are any, the paths are
    rewritten to be absolute, so that they still work when cargo is run in the
    scratch directory.
  - Before applying any mutations, check that `cargo test` succeeds in the
    scratch directory: perhaps a test is already broken, or perhaps the tree
    doesn't build when copied because it relies on relative paths to find
    dependencies, etc. This is called the "baseline" test.
  - If running more than one parallel job, make the appropriate number of
    additional scratch directories.

- For each mutation:
  - Apply the mutation to the scratch tree by patching the affected file.
  - Run `cargo build`: if this fails, the mutant is unviable, and that's ok.
  - Run `cargo test` in the tree, saving output to a log file.
  - If the the tests fail, that's good: the mutation was somehow
    caught.
  - If the tests succeed, that might mean test coverage was
    inadequate, or it might mean we accidentally generated a no-op mutation.
  - Revert the mutation to return the tree to its clean state.

The file is parsed using the [`syn`](https://docs.rs/syn) crate, but mutations
are applied textually, rather than to the token stream, so that unmutated code
retains its prior formatting, comments, line numbers, etc. This makes it
possible to show a text diff of the mutation and should make it easier to
understand any error messages from the build of the mutated code.

For more details, see [DESIGN.md](https://github.com/sourcefrog/cargo-mutants/blob/main/DESIGN.md).

# Coding agents instructions for cargo-mutants

## Building

Build the project with:

```bash
cargo build
```

For release builds:

```bash
cargo build --release
```

## Testing

cargo-mutants requires [`cargo-nextest`](https://nexte.st/) to be installed for testing.

Run tests with:

```bash
cargo test --all-features
```

Or with nextest:

```bash
cargo nextest run --all-features
```

### Test naming

Tests should have names that read like English sentences asserting a fact about behavior, like `copy_testdata_doesnt_include_build_artifacts`. Avoid "noise" words.

If the test exercises a particular test tree, option, or function, make sure that name literally occurs within the test name.

### testdata trees

Tests run against trees under `testdata/`. These are stored with `Cargo_test.toml` (instead of `Cargo.toml`) to prevent cargo from seeing them as part of the main workspace.

Always use `copy_of_testdata()` to create a temporary copy before running tests. This function automatically renames `Cargo_test.toml` to `Cargo.toml` in the copy, so the test tree becomes a valid cargo workspace.

Describe the purpose of each testdata tree in its `Cargo.toml` or `README.md`.

## Linting and formatting

Run `cargo fmt` before committing.

Run clippy checks:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

These are checked in CI and must pass.

## Documentation

Changes that have user-facing effects should be described in the appropriate section of the book (in book/src) and in NEWS.md.

## Style

Generally, variables and parameters should be the `snake_case` version of their type name: `source_tree: SourceTree`. However if that would be unclear or ambiguous, use a different name that does not repeat the type: `src: &Path, dest: &Path`.

See CONTRIBUTING.md and DESIGN.md for more detailed guidance on architecture, patterns, and contributing.

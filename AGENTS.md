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

### Nightly-only integration tests (`mutants_nightly`)

Some integration tests exercise testdata trees that use nightly-only Rust syntax (for example, custom proc-macro attributes on expressions, which require the `stmt_expr_attributes` and `proc_macro_hygiene` feature gates). To keep the default test run usable on stable Rust, these tests are gated on a custom cfg named `mutants_nightly`:

```rust
#[test]
#[cfg_attr(
    not(mutants_nightly),
    ignore = "requires --cfg=mutants_nightly and a nightly toolchain"
)]
fn check_tree_with_my_nightly_feature() { ... }
```

Their testdata enables the required nightly features under the same cfg:

```rust
#![cfg_attr(mutants_nightly, feature(stmt_expr_attributes, proc_macro_hygiene))]
```

To run these tests:

```bash
cargo +nightly nextest run --all-features \
    --config 'build.rustflags=["--cfg=mutants_nightly"]' \
    check_tree_with_my_nightly_feature
```

The integration test forwards the cfg to the cargo-mutants subprocess (and from there to the testdata's `cargo check --tests`) via `RUSTFLAGS="--cfg=mutants_nightly"` set on the subprocess env.

`mutants_nightly` is registered as a known cfg in the top-level `Cargo.toml`'s `[lints.rust]` section and in the testdata tree's `Cargo_test.toml`, so the `unexpected_cfgs` lint doesn't warn when the cfg is absent.

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

# `dangling_mod` testdata tree

This tree intentionally omits a module file to verify the discovery system continues gracefully in the face of module file warnings.

Notes:
- `#[cfg(not(test))]` allows tests to run despite the intentionally missing module file
- `lib.rs` is omitted, as the `#[cfg(not(test))]` trick does not work for the `cargo build` step in `cargo-mutants`


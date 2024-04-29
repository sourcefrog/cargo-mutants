# `dangling_mod` testdata tree

This tree intentionally references invalid module files to verify the discovery system continues gracefully in the face of module file warnings.
1. `nonexistent` - the source file does not exist
2. `outside_of_workspace` - the source file exists outside of the workspace (and is accepted by rustc) but is outside the scope of `cargo-mutants` to apply modifications

Notes:
- `#[cfg(not(test))]` allows tests to run despite the intentionally missing module file
- `lib.rs` is omitted, as the `#[cfg(not(test))]` trick does not work for the `cargo build` step in `cargo-mutants`


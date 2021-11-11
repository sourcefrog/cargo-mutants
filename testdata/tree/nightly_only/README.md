This tree only builds on nightly Rust, and can be used to check that `cargo
mutants` uses the corresponding `cargo` and `rustc` when building candidates.

For example this should fail:

    cargo +stable mutants -d ./testdata/tree/nightly_only/

and this should succeed:

    cargo +nightly mutants -d ./testdata/tree/nightly_only/

This isn't covered by an integration test because there's no guarantee the user
has both toolchains installed...

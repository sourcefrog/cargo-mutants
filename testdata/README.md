# cargo-mutants testdata

Each directory below here is a Rust tree used by the cargo-mutants tests.

In these trees, the manifest file is called `Cargo_test.toml` (rather than `Cargo.toml`) for a couple of reasons:

1. `cargo publish` excludes directories containing `Cargo.toml`, on the grounds that each crate should be published separately, but we want to include these in the published tarball so that the tests can run and succeed in an unpacked tarball. (See https://github.com/sourcefrog/cargo-mutants/issues/355.)

2. We don't want cargo to look at these crates when building or resolving dependencies for cargo-mutants itself.

Since the `--manifest-path` of Cargo commands expects the manifest to be named `Cargo.toml` we have to always copy these trees before using them. The `copy_of_testdata` helper function copies them and fixes the manifest name. Copying the tree also avoids any conflicts between concurrent or consecutive tests.

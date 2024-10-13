# testdata/symlink

This is a source tree which when built will contain a symlink in its testdata. The symlink
must exist for the tests to pass. This is used to test that cargo-mutants copies the symlinks correctly, especially on Windows.

Because `cargo publish` doesn't include symlinks, the symlink is created when the directory is copied to run the tests.

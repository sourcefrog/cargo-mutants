# tests/common

This directory is for shared code across integration tests.

Instead of being depended upon in the usual way, these files are included into test crates with `#[path="..."]` attributes.

This hack is because, as far as I can see, the tests can't depend on a common library without that crate being separately published to crates.io, which seems like overkill for a few shared functions.

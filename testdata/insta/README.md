# cargo-mutants `insta` test tree

An example of a crate that uses the [Insta](https://insta.rs) test framework.

Insta in some modes will either write `.snap.new` files into the source directory, or update existing snapshots. We don't want either of those to happen when testing mutants.

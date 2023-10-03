# package-fails

A tree with a workspace and two packages, one of which has already-failing
tests. (Let's suppose they have a non-hermetic dependency.) It should still
be possible to test the other.

See <https://github.com/sourcefrog/cargo-mutants/issues/151>.

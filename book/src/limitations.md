# Limitations, caveats, known bugs, and future enhancements

cargo-mutants behavior, output formats, command-line syntax, json output
formats, etc, may change from one release to the next.

cargo-mutants sees the AST of the tree but doesn't fully "understand" the types.

cargo-mutants reads `CARGO_ENCODED_RUSTFLAGS` and `RUSTFLAGS` environment variables, and sets `CARGO_ENCODED_RUSTFLAGS`.  It does not read `.cargo/config.toml` files, and so any rust flags set there will be ignored.

cargo-mutants does not yet understand platform-specific conditional compilation,
such as `#[cfg(target_os = "linux")]`. It will report functions for other
platforms as missed, when it should know to skip them.

## Caution on side effects

cargo-mutants builds and runs code with machine-generated modifications. This is
generally fine, but if the code under test  has side effects such as writing or
deleting files, running it with mutations might conceivably have unexpected
effects, such as deleting the wrong files, in the same way that a bug might.

If you're concerned about this, run cargo-mutants in a container or virtual
machine.

cargo-mutants never modifies the original source tree, other than writing a
`mutants.out` directory, and that can be sent elsewhere with the `--output`
option. All mutations are applied and tested in a copy of the source tree.

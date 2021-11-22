# cargo-mutants design

## Physical structure

`main.rs` -- the `cargo mutants` entry point and command-line parsing.

`lab.rs` -- a mutants "lab": manages generating and testing mutants.

`console.rs` -- colored output to the console including drawing
progress bars.
The interface to the `console` and `indicatif` crates is localized here.

`mutate.rs` -- different types of mutations we can apply.

`outcome.rs` -- the result of running a single test or build.

`output.rs` -- manages the `mutants.out` directory.

`source.rs` -- a source tree and files within it.

`textedit.rs` -- (line, column) addressing within a source file,
and edits to the content based on those addresses.

`visit.pr` -- Walk a source file's AST. The interface to the `syn` parser is
localized here.

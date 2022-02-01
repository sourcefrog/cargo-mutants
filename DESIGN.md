# cargo-mutants design

## Physical structure / source tree overview

`main.rs` -- the `cargo mutants` entry point and command-line parsing.

`console.rs` -- colored output to the console including drawing progress bars.
The interface to the `console` and `indicatif` crates is localized here.

`lab.rs` -- A mutants "lab": manages generating and testing mutants. Contains
effectively the main loop of the program: build and test every mutant.

`log_file.rs` -- Manage one log file per mutant scenario, within the output dir.

`mutate.rs` -- Different types of mutations we can apply, based on the AST from
`visit.rs`, including generating a diff for the mutation and generating a tree
with the mutation applied.

`options.rs` -- Global options for timeouts, etc.

`outcome.rs` -- The result of running a single test or build, including
distinguishing which type of command was run (check/build/test), where the log
file is, what happened (success/failure/timeout/etc), and whether a mutation was
applied.

`output.rs` -- Manages the `mutants.out` directory.

`run.rs` -- Run Cargo subprocesses, including dealing with timeouts.

`source.rs` -- A source tree and files within it.

`textedit.rs` -- A (line, column) addressing within a source file, and edits to
the content based on those addresses.

`visit.rs` -- Walk a source file's AST. The interface to the `syn` parser is
localized here.

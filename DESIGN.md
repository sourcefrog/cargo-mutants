# cargo-mutants design

## Physical structure / source tree overview

`main.rs` -- the `cargo mutants` entry point and command-line parsing.

`console.rs` -- colored output to the console including drawing progress bars.
The interface to the `console` and `indicatif` crates is localized here.

`interrupt.rs` -- Handle Ctrl-C signals by setting a global atomic flag, which
is checked during long-running operations.

`lab.rs` -- A mutants "lab": manages generating and testing mutants. Contains
effectively the main loop of the program: build and test every mutant.

`log_file.rs` -- Manage one log file per mutant scenario, within the output dir.

`mutate.rs` -- Different types of mutations we can apply, based on the AST from
`visit.rs`, including generating a diff for the mutation and generating a tree
with the mutation applied.

`options.rs` -- Global options for timeouts, etc. `main.rs` has the command line
flags; this has an internal version of the options that have a pervasive effect
through the program.

`outcome.rs` -- The result of running a single test or build, including
distinguishing which type of command was run (check/build/test), where the log
file is, what happened (success/failure/timeout/etc), and whether a mutation was
applied.

`output.rs` -- Manages the `mutants.out` directory.

`run.rs` -- Run Cargo subprocesses, including dealing with timeouts.

`source.rs` -- A source tree and files within it, including visiting each source
file to find mutations.

`textedit.rs` -- A (line, column) addressing within a source file, and edits to
the content based on those addresses.

`visit.rs` -- Walk a source file's AST. The interface to the `syn` parser is
localized here.

## Handling timeouts

Mutations can cause a program to go into an infinite (or just very long) loop:
for example we might mutate a function in `if should_terminate() { break }` to
return false.

It's also possible that the un-mutated program has a bug that makes its test
suite loop forever sometimes. Obviously this is a bug but we want cargo-mutants
to be safe and easy to use on arbitrary trees that might have bugs.

We want to handle timeouts internally for a few reasons, including:

- If one mutation hangs we still want to go on and try others. (So it's not so
  good if the `cargo mutants` process is killed by the user or a CI timeout.)

- The fact that the mutation hung is a potentially interesting signal about the
  program to report. (Possibly the user will just have to mark
  `should_terminate` as skipped, but at least they can do that once and then
  have other builds go faster.)

- For either CI or interactive use it's better if `cargo mutants` finishes in a
  bounded time.

(We are primarily concerned here with timeouts on tests; let's assume that
`cargo build` will never get stuck; if it does then the whole environment
probably has problems that need user investigation.)

The timeout for running tests is controlled by `Options::timeout`.

The timeout can be set by the user with `--timeout`, in which case it's simply
used as is. If it's not specified, it is auto-set from the time to run the
baseline tests, with a multiplier and a floor.

Detecting that a program has run too long is simple: we just watch the clock
while waiting for it to finish. Terminating it, however, is more complicated:

The immediate child process spawned by `cargo-mutants` is `cargo test ...`. This
in turn spawns its own children running the various test binaries. It is these
grandchild processes that are most likely stuck in a loop.

(It's also possible, and not unlikely, that the test binaries themselves start
children: the cargo-mutants CLI tests do this. And those great-grand-children
might get stuck. But the same logic applies.)

    cargo mutants ....
      cargo test ...
        target/debug/someprog_api_test
        target/debug/someprog_cli_test
          target/debug/someprog ...

When we decide to stop the long-running test, we need to terminate the whole
tree of processes. Unix provides a "process group" concept for doing this: we
put the immediate child in a new process group, and then all its descendents
will also be in that process group. We can stop the whole lot using `killpg`.

However, the test processes are then _not_ in cargo-mutants's process group. So
if the user hits ctrl-c on `cargo mutants`, that signal will not get to the test
processes: cargo mutants would stop but the test process that's actually chewing
up the CPU will continue.

Therefore we need to also intercept the signal to cargo-mutants and manually
pass it on to the subprocess group.

## Output directory handling

Various output files, including the text output from all the cargo commands are
written into `mutants.out` within the directory specified by `--output`, or by
default the source directory.

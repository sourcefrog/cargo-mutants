# cargo-mutants design

See also [CONTRIBUTING.md](CONTRIBUTING.md) for more advice on style, approach, etc.

## Physical structure / source tree overview

`main.rs` -- the `cargo mutants` entry point and command-line parsing.

`cargo.rs` -- Run Cargo subprocesses, including dealing with timeouts and understanding Cargo output.

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

`scenario.rs` -- Each of the build/test cycles is a "scenario": either building the source tree, testing the baseline, or testing a mutant.

`source.rs` -- A source tree and files within it, including visiting each source
file to find mutations.

`textedit.rs` -- A (line, column) addressing within a source file, and edits to
the content based on those addresses.

`visit.rs` -- Walk a source file's AST. The interface to the `syn` parser is
localized here.

## Relative dependencies

After copying the tree, cargo-mutants scans the top-level `Cargo.toml` and any
`.cargo/config.toml` for relative dependencies. If there are any, the paths are
rewritten to be absolute, so that they still work when cargo is run in the
scratch directory.

## Enumerating the source tree

A source tree may be a single crate or a Cargo workspace. There are several levels of nesting:

* A workspace contains one or more packages.
* A package contains one or more targets.
* Each target names one (or possibly-more) top level source files, whose directories are walked to find more source files.
* Each source file contains some functions.
* For each function we generate some mutants.

The name of the containing package is passed through to the `SourceFile` and the `Mutant` objects.

For source tree and baseline builds and tests, we pass Cargo `--workspace` to build and test everything. For mutant builds and tests, we pass `--package` to build and test only the package containing the mutant, on the assumption that each mutant should be caught by its own package's tests.

Currently source files are discovered by finding any `bin` or `lib` targets in the package, then taking every `*.rs` file in the same directory as their top-level source file. (This is a bit approximate and will pick up files that might not actually be referenced by a `mod` statement, so may change in the future.)

We may later mutate at a granularity smaller than a single function, for example by cutting out an `if` statement or a loop, but that is not yet implemented. (<https://github.com/sourcefrog/cargo-mutants/issues/73>)

## Handling timeouts

Mutations can cause a program to go into an infinite (or just very long) loop:
for example we might mutate a function in `if should_terminate() { break }` to
return false.

It's also possible that the un-mutated program has a bug that makes its test
suite loop forever sometimes. Obviously this is a bug but we want cargo-mutants
to be safe and easy to use on arbitrary trees that might have bugs.

We want to handle timeouts internally for a few reasons, including:

* If one mutation hangs we still want to go on and try others. (So it's not so
  good if the `cargo mutants` process is killed by the user or a CI timeout.)

* The fact that the mutation hung is a potentially interesting signal about the
  program to report. (Possibly the user will just have to mark
  `should_terminate` as skipped, but at least they can do that once and then
  have other builds go faster.)

* For either CI or interactive use it's better if `cargo mutants` finishes in a
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

## Handling strict lints

Some trees are configured so that any unused variable is an error. This is a reasonable choice to keep the tree very clean in CI, but if unhandled it would be a problem for cargo mutants. Many mutants -- in fact at the time of writing all generated mutants -- ignore function parameters and return a static value. Rejecting them due to the lint failure is a missed opportunity to consider a similar but more subtle potential bug.

Therefore when running `rustc` we configure all warnings off, with `--cap-lints`.

## Testing

Cargo-mutants is primarily tested on its public interface, which is the command line. These tests live in `tests/cli` and generally have the form of:

1. Make a copy of a `testdata` tree, so that it's not accidentally modified.
2. Run a `cargo-mutants` command on it.
3. Inspect the stdout, return code, or `mutants.out`.

`cargo-mutants` runs as a subprocess of the test process so that we get the most realistic view of its behavior. In some cases it is run via the `cargo` command to test that this level of indirection works properly.

### `testdata` trees

The primary means of testing is Rust source trees under `testdata/tree`: you can copy an existing tree and modify it to show the new behavior that you want to test.

A selection of test trees are available for testing different scenarios. If there is an existing suitable tree, please use it. If you need to test a situation that is not covered yet, please add a new tree.

Please describe the purpose of the testdata tree inside the tree, either in `Cargo.toml` or a `README.md` file.

To make a new tree you can copy an existing tree, but make sure to change the package name in its `Cargo.toml`.

All the trees need to be excluded from the overall workspace in the top-level `Cargo.toml`.

### `--list` tests

There is a general test that runs `cargo mutants --list` on each tree and compares the result to an Insta snapshot, for both the text and json output formats.

Many features can be tested adequately by only looking at the list of mutants produced, with no need to actually test the mutants. In this case the generic list tests might be enough.

### Unit tests

Although we primarily want to test the public interface (which is the command line), unit tests can be added in a `mod test {}` within the source tree for any behavior that is inconvenient to exercise from the command line.

## UI Style

Always print paths with forward slashes, even on Windows. Use `path_slash`.

## Logging

cargo-mutants writes two types of log files into the `mutants.out` directory:

Within the `log` directory, there is a file for each mutant scenario. This mostly contains the output from the Cargo commands, interspersed with some lines showing what command was run and the outcome. This is opened in append mode so that both cargo-mutants and cargo can write to it. This log is watched and the last line is shown in the progress bar to indicate what's going on.

Also, a file `mutants.out/debug.log` is written using [tracing](https://docs.rs/tracing) and written using the tracing macros. This contains more detailed information about what's happening in cargo-mutants itself.

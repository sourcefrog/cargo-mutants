# Display and output

cargo-mutants writes a list of missed or timed-out mutants to stderr, and optionally mutants that were caught (with `--caught`) or failed to build (with `--unviable`) to stdout. It writes error or debug messages to stderr.

The following options control what is printed to stdout and stderr.

`-v`, `--caught`: Also print mutants that were caught by tests.

`-V`, `--unviable`: Also print mutants that failed `cargo build`.

`--no-times`: Don't print elapsed times. (This is intended mostly to make the output more stable for testing.)

## Colors

`--colors=always|never|auto`: Control whether to use colors in output. The default is `auto`, which will write colors if the output is a terminal that supports colors. Color support is detected independently for stdout and stderr, so you should still see colors on stderr if stdout is redirected.

The same values can be set with the `CARGO_TERM_COLOR` environment variable, which is respected by many Cargo commands.

cargo-mutants also respects the [`NO_COLOR`](https://no-color.org/) and [`CLICOLOR_FORCE`](https://bixense.com/clicolors/) environment variables. If they are set to a value other than `0` then colors will be disabled or enabled regardless of any other settings.

## Debug trace

`-L`, `--level`, and `$CARGO_MUTANTS_TRACE_LEVEL`: set the verbosity of trace output to stderr. The default is `info`, and it can be increased to `debug` or `trace`.

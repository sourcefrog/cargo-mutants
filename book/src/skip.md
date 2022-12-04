# Skipping untestable code

Some functions may be inherently hard to cover with tests, for example if:

* Generated mutants cause tests to hang.
* You've chosen to test the functionality by human inspection or some higher-level integration tests.
* The function has side effects or performance characteristics that are hard to test.
* You've decided the function is not important to test.

## Skipping function with an attribute

To mark functions so they are not mutated:

1. Add a Cargo dependency on the [mutants](https://crates.io/crates/mutants)
   crate, version "0.0.3" or later. (This must be a regular `dependency` not a
   `dev-dependency`, because the annotation will be on non-test code.)

2. Mark functions with `#[mutants::skip]` or other attributes containing
   `mutants::skip` (e.g. `#[cfg_attr(test, mutants::skip)]`).

The `mutants` create is tiny and the attribute has no effect on the compiled
code. It only flags the function for cargo-mutants.

**Note:** Currently, `cargo-mutants` does not (yet) evaluate attributes like
`cfg_attr`, it only looks for the sequence `mutants::skip` in the attribute.

You may want to also add a comment explaining why the function is skipped.

**TODO**: Explain why `cfg_attr`.

For example:

```rust
use std::time::{Duration, Instant};

/// If mutated to return false, the program will spin forever.
#[cfg_attr(test, mutants::skip)] // causes a hang
fn should_stop() -> bool {
    true
}

pub fn controlled_loop() {
    let start = Instant::now();
    for i in 0.. {
        println!("{}", i);
        if should_stop() {
            break;
        }
        if start.elapsed() > Duration::from_secs(60 * 5) {
            panic!("timed out");
        }
    }
}

mod test {
    #[test]
    fn controlled_loop_terminates() {
        super::controlled_loop()
    }
}
```

## Skipping files

**Note:** Rust's "inner macro attributes" feature is currently unstable, so
`#![mutants::skip]` can't be used in module scope or on a `mod` statement.

However, you can use the `exclude_globs` key in
[`.cargo/mutants.toml`](config.md), or the `--exclude` command-line option, to exclude files.

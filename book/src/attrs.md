# Skipping functions with an attribute

To mark functions as skipped, so they are not mutated:

1. Add a Cargo dependency on the [mutants](https://crates.io/crates/mutants)
   crate, version "0.0.3" or later. (This must be a regular `dependency` not a
   `dev-dependency`, because the annotation will be on non-test code.)

2. Mark functions with `#[mutants::skip]` or other attributes containing
   `mutants::skip` (e.g. `#[cfg_attr(test, mutants::skip)]`).

The `mutants` create is tiny and the attribute has no effect on the compiled
code. It only flags the function for cargo-mutants. However, you can avoid the
dependency by using the slightly longer `#[cfg_attr(test, mutants::skip)]` form.

**Note:** Currently, `cargo-mutants` does not (yet) evaluate attributes like
`cfg_attr`, it only looks for the sequence `mutants::skip` in the attribute.

You may want to also add a comment explaining why the function is skipped.

For example:

```rust
use std::time::{Duration, Instant};

/// Returns true if the program should stop
#[cfg_attr(test, mutants::skip)] // Returning false would cause a hang
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

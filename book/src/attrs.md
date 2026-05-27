# Skipping mutations with an attribute

To mark items as skipped, so they are not mutated:

1. Add a Cargo dependency on the [mutants](https://crates.io/crates/mutants)
   crate, version "0.0.3" or later. (This must be a regular `dependency` not a
   `dev-dependency`, because the annotation will be on non-test code.)

2. Mark items with `#[mutants::skip]`, or with `mutants::skip` nested inside a
   `cfg_attr` (e.g. `#[cfg_attr(test, mutants::skip)]`).

The `mutants` crate is tiny and the attribute has no effect on the compiled
code. It only flags the item for cargo-mutants.

**Note:** `cargo-mutants` does not evaluate the `cfg_attr` condition; the
inner `mutants::skip` is always honoured regardless of whether the condition
would hold during compilation.

You may want to also add a comment explaining why the item is skipped.

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

## Scope

`#[mutants::skip]` can be placed on:

- **Functions** — applies to all mutations within that function.
- **`impl` blocks** — applies to all methods within the block.
- **`trait` blocks** — applies to all default method implementations.
- **`mod` blocks** — applies to all items within the module.
- **Files** (as an inner attribute `#![mutants::skip]`) — applies to the entire file.
- **Expressions** that can syntactically carry an outer attribute, including
  `match`, struct literal (`Foo { ... }`), call (`foo(...)`), method-call
  (`x.foo(...)`), and unary expressions (`!x`, `-x`) — applies to the
  expression and everything nested inside it.

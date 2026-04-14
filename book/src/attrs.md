# Skipping functions with an attribute

To mark functions as skipped, so they are not mutated:

1. Add a Cargo dependency on the [mutants](https://crates.io/crates/mutants)
   crate, version "0.0.3" or later. (This must be a regular `dependency` not a
   `dev-dependency`, because the annotation will be on non-test code.)

2. Mark functions with `#[mutants::skip]` or other attributes containing
   `mutants::skip` (e.g. `#[cfg_attr(test, mutants::skip)]`).

The `mutants` crate is tiny and the attribute has no effect on the compiled
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

## Excluding specific mutations with an attribute

If `#[mutants::skip]` is too broad (it disables _all_ mutations on a function)
you can use `#[mutants::exclude_re("pattern")]` to exclude only mutations
whose name matches a regex, while keeping the rest.

The regex is matched against the full mutant name (the same string shown by
`cargo mutants --list`), using the same syntax as `--exclude-re` on the command
line.

For example, to keep all mutations except the "replace with ()" return-value
mutation:

```rust
#[mutants::exclude_re("with \\(\\)")]
fn do_something(x: i32) -> i32 {
    x + 1
}
```

Multiple attributes can be applied to exclude several patterns:

```rust
#[mutants::exclude_re("with 0")]
#[mutants::exclude_re("with 1")]
fn compute(a: i32, b: i32) -> i32 {
    a + b
}
```

As with `mutants::skip`, cargo-mutants also looks for `mutants::exclude_re`
within other attributes such as `cfg_attr`, without evaluating the outer
attribute:

```rust
#[cfg_attr(test, mutants::exclude_re("replace .* -> bool"))]
fn is_valid(&self) -> bool {
    // ...
    true
}
```

### Scope

`#[mutants::exclude_re]` can be placed on:

- **Functions** — applies to all mutations within that function.
- **`impl` blocks** — applies to all methods within the block.
- **`trait` blocks** — applies to all default method implementations.
- **`mod` blocks** — applies to all items within the module.
- **Files** (as an inner attribute `#![mutants::exclude_re("...")]`) — applies to the entire file.

Patterns from outer scopes are inherited: if an `impl` block excludes a pattern,
all methods inside also exclude that pattern, in addition to any patterns on the
methods themselves.

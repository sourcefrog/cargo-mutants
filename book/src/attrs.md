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
  block (`{ ... }`), `match`, struct literal (`Foo { ... }`), call
  (`foo(...)`), method-call (`x.foo(...)`), and unary expressions (`!x`,
  `-x`) — applies to the expression and everything nested inside it.

## Hiding the attribute from rustc with `cfg_attr(any(), ...)`

Some uses of `#[mutants::skip]` are inconvenient or impossible to apply
directly:

- Stable Rust does not accept custom proc-macro attributes on
  expressions; placing `#[mutants::skip]` directly on a block or other
  expression only compiles on nightly (it requires the unstable
  `stmt_expr_attributes` and `proc_macro_hygiene` features).
- You may not want a crate-wide dependency on the `mutants` crate just
  to suppress a few mutants.

For both cases, wrap the attribute in a `cfg_attr` whose condition is
always false:

```rust,ignore
fn frobnicate(x: i32) -> i32 {
    #[cfg_attr(any(), mutants::skip)]
    {
        x + 1
    }
}
```

`any()` is built into rustc and, with no operands, always evaluates to
false. The compiler therefore strips the whole `cfg_attr` away and never
expands the inner `mutants::skip` proc-macro attribute. cargo-mutants
parses the source independently and still recognises the inner
`mutants::skip` directive and applies the suppression.

Because the inner attribute is never expanded, code that only uses the
`cfg_attr(any(), ...)` form **does not need a dependency on the
`mutants` crate at all**. (A direct `#[mutants::skip]` does still need
the dependency so that rustc can resolve the attribute path.)

Using `any()` rather than a made-up cfg name (such as `cfg(mutants)` or
`cfg(never)`) also avoids the `unexpected_cfgs` lint that rustc emits
for cfg names it does not know about.

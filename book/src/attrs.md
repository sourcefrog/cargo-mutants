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
would hold during compilation. This may change in future versions.

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
  Note that the `#[mutants::skip]` macro on expressions requires the
  unstable `stmt_expr_attributes` and `proc_macro_hygiene` features, so
  expression-level `#[mutants::skip]` is currently only usable on a
  nightly Rust toolchain.

## Excluding specific mutations with an attribute

If `#[mutants::skip]` is too broad (it disables _all_ mutations on a function)
you can use `#[mutants::exclude_re("pattern")]` to exclude only mutations
whose name matches a regex, while keeping the rest.

`#[mutants::exclude_re]` is available in the [mutants](https://crates.io/crates/mutants)
crate from version `0.0.5` onwards.

The regex is matched against the full mutant name (the same string shown by
`cargo mutants --list`), using the same syntax as `--exclude-re` on the command
line.

For example, to keep all mutations on an `i32`-returning function except the
"replace ... -> i32 with 0" return-value mutation:

```rust
#[mutants::exclude_re("with 0")]
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

As with `mutants::skip`, cargo-mutants also recognises `mutants::exclude_re`
when it is nested inside a `cfg_attr`. The cfg condition is *not* evaluated —
the attribute is always honoured regardless of whether the condition would
hold during compilation.

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
- **Expressions** that can syntactically carry an outer attribute, including
  `match`, struct literal (`Foo { ... }`), call (`foo(...)`), method-call
  (`x.foo(...)`), and unary expressions (`!x`, `-x`) — applies to the mutations
  generated for that expression and any expressions nested inside it.

Patterns from outer scopes are inherited: if an `impl` block excludes a pattern,
all methods inside also exclude that pattern, in addition to any patterns on the
methods themselves.

//! Test how cargo-mutants handles a tree that doesn't build.
//!
//! This version is at least parseable as a Rust AST (so that we can list mutants) but it won't typecheck.

fn try_value_coercion() -> String {
    "1" + 2 // Doesn't work in Rust: just as well!
}

#[test]
fn add_string_and_integer() {
    assert_eq!(try_value_coercion(), "3"); // probably not
}

//! Verify that `#[mutants::skip]` placed directly on a block expression
//! `{ ... }` inside a function body suppresses every mutant generated
//! inside that block, while mutants in surrounding code are still
//! produced as usual.
//!
//! Custom proc-macro attributes on expressions and block expressions
//! require the unstable `stmt_expr_attributes` and `proc_macro_hygiene`
//! features, which are only available on nightly Rust. We therefore
//! gate the feature attributes on `cfg(mutants_nightly)`. The
//! integration test that consumes this tree is `#[ignore]`d unless
//! `mutants_nightly` is set, and the same cfg is forwarded to the
//! `cargo check --tests` subprocess that cargo-mutants runs against
//! this tree, so the feature gates kick in there too. See AGENTS.md
//! for the convention.

#![cfg_attr(
    mutants_nightly,
    feature(stmt_expr_attributes, proc_macro_hygiene)
)]

/// `#[mutants::skip]` on a block used as a statement.
///
/// The `+=` and `+` operators inside the annotated block must produce no
/// mutants. The `+` and `-` on the tail expression must still be mutated
/// as usual.
pub fn statement_position(a: i32, b: i32, c: i32, d: i32) -> i32 {
    let mut total = 0;
    #[mutants::skip]
    {
        total += a + b;
    }
    total + (c - d)
}

/// `#[mutants::skip]` on an unlabeled block used as the function's tail
/// expression.
///
/// Every operator inside the annotated block — `>`, `+`, `-` — must
/// produce no mutants.
pub fn tail_block(a: i32, b: i32, c: i32) -> i32 {
    let _ = 0;
    #[mutants::skip]
    {
        if a > b {
            a + b
        } else {
            a - c
        }
    }
}

/// `#[mutants::skip]` on a labeled block used as the function's tail
/// expression.
///
/// Every operator inside the annotated block — `>`, `+`, `-` — must
/// produce no mutants.
pub fn labeled_block(a: i32, b: i32, c: i32) -> i32 {
    #[mutants::skip]
    'block: {
        if a > b {
            break 'block a + b;
        }
        a - c
    }
}

/// Sibling un-annotated block — mutants here must NOT be suppressed.
///
/// Paired with `statement_position` etc. above, this confirms the
/// suppression is scoped to the annotated block and does not leak into
/// surrounding code.
pub fn unannotated_sibling(a: i32, b: i32) -> i32 {
    let x = { a * b };
    let y = { a / b };
    x | y
}

//! Verify that `#[mutants::skip]` on a block expression `{ ... }`
//! suppresses every mutant generated inside that block, while mutants
//! in surrounding code are still produced as usual.
//!
//! On stable Rust, custom (proc-macro) attributes on expressions and
//! block expressions are gated behind the unstable `stmt_expr_attributes`
//! and `proc_macro_hygiene` features. To keep this testdata compilable on
//! stable, we use the `#[cfg_attr(any(), mutants::skip)]` wrapping: the
//! built-in `any()` cfg predicate with no operands evaluates to false at
//! compile time, so rustc strips the whole `cfg_attr` and never sees the
//! inner `mutants::skip` proc-macro attribute. cargo-mutants still
//! recognises it via `visit::attr_is_mutants_skip` and applies the
//! suppression. The unit tests under
//! `src/visit/test/skip_attr_expr_block.rs` additionally cover the direct
//! `#[mutants::skip]` form, which only needs to parse through syn.

/// `#[mutants::skip]` on a block used as a statement.
///
/// The `+=` and `+` operators inside the annotated block must produce no
/// mutants. The `+` and `-` on the tail expression must still be mutated
/// as usual.
pub fn statement_position(a: i32, b: i32, c: i32, d: i32) -> i32 {
    let mut total = 0;
    #[cfg_attr(any(), mutants::skip)]
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
    #[cfg_attr(any(), mutants::skip)]
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
    #[cfg_attr(any(), mutants::skip)]
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


//! Verify that `#[mutants::skip]` placed directly on expressions other
//! than block expressions suppresses every mutant generated inside the
//! annotated expression, while sibling expressions of the same genre in
//! the same function are still mutated as usual.
//!
//! Custom proc-macro attributes on expressions and statements require
//! the unstable `stmt_expr_attributes` and `proc_macro_hygiene`
//! features, which are only available on nightly Rust. We therefore
//! gate the feature attributes on `cfg(mutants_nightly)`. The
//! integration test that consumes this tree is `#[ignore]`d unless
//! `mutants_nightly` is set, and the same cfg is forwarded to the
//! `cargo check --tests` subprocess that cargo-mutants runs against
//! this tree, so the feature gates kick in there too. See AGENTS.md
//! for the convention.
//!
//! Block-expression skip is covered separately by the `skip_attr_block`
//! testdata tree.

#![cfg_attr(
    mutants_nightly,
    feature(stmt_expr_attributes, proc_macro_hygiene)
)]

/// `#[mutants::skip]` on a call expression.
///
/// The `+` operator inside the annotated call's argument must produce
/// no mutants. The `-` operator inside the un-annotated sibling call
/// must still be mutated as usual.
pub fn call_expr(a: i32, b: i32, c: i32, d: i32) {
    #[mutants::skip]
    helper(a + b);
    helper(c - d);
}

/// `#[mutants::skip]` on a method-call expression.
///
/// The `+` operator inside the annotated method call's argument must
/// produce no mutants. The `-` operator inside the un-annotated sibling
/// method call must still be mutated as usual.
pub fn method_call_expr(s: &Holder, a: i32, b: i32, c: i32, d: i32) {
    #[mutants::skip]
    s.frob(a + b);
    s.frob(c - d);
}

/// `#[mutants::skip]` on a `match` expression.
///
/// The annotated match must produce no arm-deletion, guard-replacement
/// or operator mutants. The un-annotated sibling match must still
/// produce all of those mutants.
pub fn match_expr(x: i32, y: i32) {
    let _ = #[mutants::skip]
    match x {
        0 => "zero",
        n if n > y => "gt",
        _ => "other",
    };
    let _ = match x {
        0 => "zero",
        n if n > y => "gt",
        _ => "other",
    };
}

/// `#[mutants::skip]` on a struct-literal expression.
///
/// The annotated struct literal must produce no field-deletion mutants.
/// The un-annotated sibling literal must still produce them.
pub fn struct_expr() {
    let _ = #[mutants::skip]
    Settings {
        enabled: true,
        count: 1,
        ..Default::default()
    };
    let _ = Settings {
        enabled: true,
        count: 1,
        ..Default::default()
    };
}

/// `#[mutants::skip]` on a unary `!` expression.
///
/// The annotated `!b` must produce no `delete !` mutant. The
/// un-annotated sibling `!b` must still produce one.
pub fn unary_expr(b: bool) {
    let _ = #[mutants::skip] !b;
    let _ = !b;
}

#[derive(Default)]
pub struct Settings {
    pub enabled: bool,
    pub count: i32,
}

pub struct Holder;

impl Holder {
    pub fn frob(&self, _x: i32) {}
}

fn helper(_x: i32) {}

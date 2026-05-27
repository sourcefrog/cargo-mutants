//! Tests for expression-level `#[mutants::exclude_re(...)]` on call
//! expressions.

use indoc::indoc;
use test_log::test;

use crate::Options;
use crate::visit::mutate_source_str;

/// `exclude_re` on a call expression must cover mutants generated inside
/// that call's arguments (nested expression mutants), while siblings in
/// the enclosing function are unaffected.
#[test]
fn exclude_re_attr_on_call_expr_filters_nested_arg_mutants() {
    let options = Options::default();
    let mutants = mutate_source_str(
        indoc! {r#"
            fn helper(_x: i32) {}

            fn driver(a: i32, b: i32, c: i32, d: i32) {
                #[mutants::exclude_re("replace \\+ with")]
                helper(a + b);
                helper(c - d);
            }
        "#},
        &options,
    )
    .unwrap();
    let names: Vec<String> = mutants.iter().map(|m| m.name(false)).collect();

    // The `+` inside the annotated call is filtered out.
    assert!(
        !names.iter().any(|n| n.contains("replace + with")),
        "`+` inside annotated call should be filtered: {names:?}"
    );
    // The `-` in the sibling call still generates mutants.
    assert!(
        names.iter().any(|n| n.contains("replace - with")),
        "`-` in unannotated sibling call should remain: {names:?}"
    );
}

//! Tests that `#[mutants::skip]` placed on a call expression suppresses
//! mutants generated inside that call's arguments, while sibling calls in
//! the same function remain mutated.

use indoc::indoc;
use test_log::test;

use crate::Options;
use crate::visit::mutate_source_str;

#[test]
fn skip_attr_on_call_expression_suppresses_nested_arg_mutants() {
    let mutants = mutate_source_str(
        indoc! {r#"
            fn helper(_x: i32) {}

            fn driver(a: i32, b: i32, c: i32, d: i32) {
                #[mutants::skip]
                helper(a + b);
                helper(c - d);
            }
        "#},
        &Options::default(),
    )
    .unwrap();
    let names: Vec<String> = mutants.iter().map(|m| m.name(false)).collect();

    assert!(
        !names.iter().any(|n| n.contains("replace + with")),
        "`+` inside skipped call should not produce mutants: {names:?}"
    );
    assert!(
        names.iter().any(|n| n.contains("replace - with")),
        "`-` in the unannotated sibling call should still produce mutants: {names:?}"
    );
}

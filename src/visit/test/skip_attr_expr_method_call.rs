//! Tests that `#[mutants::skip]` placed on a method-call expression
//! suppresses mutants generated inside the receiver and arguments, while
//! sibling method calls in the same function remain mutated.

use indoc::indoc;
use test_log::test;

use crate::Options;
use crate::visit::mutate_source_str;

#[test]
fn skip_attr_on_method_call_expression_suppresses_nested_mutants() {
    let mutants = mutate_source_str(
        indoc! {r#"
            struct S;

            impl S {
                fn frob(&self, _x: i32) {}
            }

            fn driver(s: &S, a: i32, b: i32, c: i32, d: i32) {
                #[mutants::skip]
                s.frob(a + b);
                s.frob(c - d);
            }
        "#},
        &Options::default(),
    )
    .unwrap();
    let names: Vec<String> = mutants.iter().map(|m| m.name(false)).collect();

    assert!(
        !names.iter().any(|n| n.contains("replace + with")),
        "`+` inside skipped method call should not produce mutants: {names:?}"
    );
    assert!(
        names.iter().any(|n| n.contains("replace - with")),
        "`-` in the unannotated sibling method call should still produce mutants: {names:?}"
    );
}

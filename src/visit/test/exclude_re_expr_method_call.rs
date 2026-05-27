//! Tests for expression-level `#[mutants::exclude_re(...)]` on method-call
//! expressions.

use indoc::indoc;
use test_log::test;

use crate::Options;
use crate::visit::mutate_source_str;

/// `exclude_re` on a method-call expression must cover mutants generated
/// inside that method call's receiver/arguments, while siblings remain.
#[test]
fn exclude_re_attr_on_method_call_filters_nested_mutants() {
    let options = Options::default();
    let mutants = mutate_source_str(
        indoc! {r#"
            struct S;

            impl S {
                fn frob(&self, _x: i32) {}
            }

            fn driver(s: &S, a: i32, b: i32, c: i32, d: i32) {
                #[mutants::exclude_re("replace \\+ with")]
                s.frob(a + b);
                s.frob(c - d);
            }
        "#},
        &options,
    )
    .unwrap();
    let names: Vec<String> = mutants.iter().map(|m| m.name(false)).collect();

    // The `+` inside the annotated method call is filtered out.
    assert!(
        !names.iter().any(|n| n.contains("replace + with")),
        "`+` inside annotated method call should be filtered: {names:?}"
    );
    // The `-` in the sibling method call still generates mutants.
    assert!(
        names.iter().any(|n| n.contains("replace - with")),
        "`-` in unannotated sibling method call should remain: {names:?}"
    );
}

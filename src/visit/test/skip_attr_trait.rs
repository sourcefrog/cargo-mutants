//! Tests that `#[mutants::skip]` on a trait declaration suppresses mutants
//! generated from the trait's default-method bodies.

use indoc::indoc;
use test_log::test;

use crate::Options;
use crate::visit::mutate_source_str;

#[test]
fn skip_attr_on_trait_declaration_suppresses_default_method_mutants() {
    let mutants = mutate_source_str(
        indoc! {r#"
            #[mutants::skip]
            trait Calc {
                fn add(&self, a: i32, b: i32) -> i32 {
                    a + b
                }

                fn sub(&self, a: i32, b: i32) -> i32 {
                    a - b
                }
            }

            fn outside(a: i32, b: i32) -> i32 {
                a * b
            }
        "#},
        &Options::default(),
    )
    .unwrap();
    let names: Vec<String> = mutants.iter().map(|m| m.name(false)).collect();

    assert!(
        !names
            .iter()
            .any(|n| n.contains("Calc::add") || n.contains("Calc::sub")),
        "trait default methods inside skipped trait should produce no mutants: {names:?}"
    );
    assert!(
        names.iter().any(|n| n.contains("outside")),
        "sibling function outside the trait should still produce mutants: {names:?}"
    );
}

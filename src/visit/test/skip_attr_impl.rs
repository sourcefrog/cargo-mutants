//! Tests that `#[mutants::skip]` on an impl block declaration suppresses
//! mutants for every method inside it.

use indoc::indoc;
use test_log::test;

use crate::Options;
use crate::visit::mutate_source_str;

#[test]
fn skip_attr_on_impl_block_suppresses_all_methods() {
    let mutants = mutate_source_str(
        indoc! {r#"
            struct S;

            #[mutants::skip]
            impl S {
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
            .any(|n| n.contains("S::add") || n.contains("S::sub")),
        "methods inside skipped impl should produce no mutants: {names:?}"
    );
    assert!(
        names.iter().any(|n| n.contains("outside")),
        "sibling function outside the impl should still produce mutants: {names:?}"
    );
}

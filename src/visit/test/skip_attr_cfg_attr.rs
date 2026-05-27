//! Tests that `#[cfg_attr(<cond>, mutants::skip)]` is honoured at scopes
//! other than top-level `fn` — for example on an `impl` block and on a
//! `mod`. The visitor ignores the cfg condition and always treats the
//! attribute as an instruction to skip, matching the documented behaviour
//! of `#[cfg_attr(test, mutants::skip)]` on functions.

use indoc::indoc;
use test_log::test;

use crate::Options;
use crate::visit::mutate_source_str;

#[test]
fn cfg_attr_mutants_skip_on_impl_block_suppresses_all_methods() {
    let mutants = mutate_source_str(
        indoc! {r#"
            struct S;

            #[cfg_attr(test, mutants::skip)]
            impl S {
                fn add(&self, a: i32, b: i32) -> i32 {
                    a + b
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
        !names.iter().any(|n| n.contains("S::add")),
        "cfg_attr(mutants::skip) on impl block should suppress its methods: {names:?}"
    );
    assert!(
        names.iter().any(|n| n.contains("outside")),
        "sibling function should still produce mutants: {names:?}"
    );
}

#[test]
fn cfg_attr_mutants_skip_on_mod_suppresses_inner_items() {
    let mutants = mutate_source_str(
        indoc! {r#"
            #[cfg_attr(test, mutants::skip)]
            mod inner {
                pub fn add(a: i32, b: i32) -> i32 {
                    a + b
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
        !names.iter().any(|n| n.contains("inner::add")),
        "cfg_attr(mutants::skip) on mod should suppress its items: {names:?}"
    );
    assert!(
        names.iter().any(|n| n.contains("outside")),
        "sibling function outside the mod should still produce mutants: {names:?}"
    );
}

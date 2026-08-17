//! Tests that `#[mutants::skip]` on a block expression `{ ... }` suppresses
//! mutants generated inside that block, while sibling code in the same
//! function remains mutated.

use indoc::indoc;
use test_log::test;

use crate::Options;
use crate::visit::mutate_source_str;

#[test]
fn skip_attr_on_statement_position_block_suppresses_nested_mutants() {
    let mutants = mutate_source_str(
        indoc! {r#"
            fn driver(a: i32, b: i32, c: i32, d: i32) {
                #[mutants::skip]
                {
                    let _ = a + b;
                }
                let _ = c - d;
            }
        "#},
        &Options::default(),
    )
    .unwrap();
    let names: Vec<String> = mutants.iter().map(|m| m.name(false)).collect();

    assert!(
        !names.iter().any(|n| n.contains("replace + with")),
        "`+` inside skipped block should not produce mutants: {names:?}"
    );
    assert!(
        names.iter().any(|n| n.contains("replace - with")),
        "`-` in the unannotated sibling code should still produce mutants: {names:?}"
    );
}

#[test]
fn skip_attr_on_expression_position_block_suppresses_nested_mutants() {
    let mutants = mutate_source_str(
        indoc! {r#"
            fn driver(a: i32, b: i32, c: i32, d: i32) -> i32 {
                let x = #[mutants::skip] {
                    a + b
                };
                let y = {
                    c - d
                };
                x | y
            }
        "#},
        &Options::default(),
    )
    .unwrap();
    let names: Vec<String> = mutants.iter().map(|m| m.name(false)).collect();

    assert!(
        !names.iter().any(|n| n.contains("replace + with")),
        "`+` inside skipped block should not produce mutants: {names:?}"
    );
    assert!(
        names.iter().any(|n| n.contains("replace - with")),
        "`-` in the unannotated sibling block should still produce mutants: {names:?}"
    );
    assert!(
        names.iter().any(|n| n.contains("replace | with")),
        "`|` in the unannotated tail expression should still produce mutants: {names:?}"
    );
}

#[test]
fn skip_attr_on_block_suppresses_all_genres_within() {
    let mutants = mutate_source_str(
        indoc! {r#"
            fn pick(x: i32, y: i32) -> &'static str {
                #[mutants::skip]
                {
                    let _ = !true;
                    match x {
                        0 => "zero",
                        n if n > y => "gt",
                        _ => "other",
                    }
                }
            }
        "#},
        &Options::default(),
    )
    .unwrap();
    let names: Vec<String> = mutants.iter().map(|m| m.name(false)).collect();

    assert!(
        !names.iter().any(|n| n.contains("delete !")),
        "unary mutants inside skipped block should be suppressed: {names:?}"
    );
    assert!(
        !names.iter().any(|n| n.contains("delete match arm")),
        "match arm deletion mutants inside skipped block should be suppressed: {names:?}"
    );
    assert!(
        !names.iter().any(|n| n.contains("replace match guard")),
        "match guard mutants inside skipped block should be suppressed: {names:?}"
    );
    assert!(
        !names.iter().any(|n| n.contains("replace > with")),
        "binary mutants inside skipped block should be suppressed: {names:?}"
    );
}

#[test]
fn skip_attr_on_labeled_block_suppresses_nested_mutants() {
    let mutants = mutate_source_str(
        indoc! {r#"
            fn driver(a: i32, b: i32) -> i32 {
                #[mutants::skip]
                'block: {
                    if a > b {
                        break 'block a + b;
                    }
                    a - b
                }
            }
        "#},
        &Options::default(),
    )
    .unwrap();
    let names: Vec<String> = mutants.iter().map(|m| m.name(false)).collect();

    assert!(
        !names.iter().any(|n| n.contains("replace + with")),
        "`+` inside skipped labeled block should not produce mutants: {names:?}"
    );
    assert!(
        !names.iter().any(|n| n.contains("replace - with")),
        "`-` inside skipped labeled block should not produce mutants: {names:?}"
    );
    assert!(
        !names.iter().any(|n| n.contains("replace > with")),
        "`>` inside skipped labeled block should not produce mutants: {names:?}"
    );
}

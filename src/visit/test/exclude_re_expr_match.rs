//! Tests for expression-level `#[mutants::exclude_re(...)]` on match
//! expressions.

use indoc::indoc;
use test_log::test;

use crate::Options;
use crate::visit::mutate_source_str;

/// `exclude_re` on a match expression with a catch-all must suppress
/// the arm-deletion mutants generated for that match.
#[test]
fn exclude_re_attr_on_match_expr_filters_arm_deletions() {
    let options = Options::default();
    let mutants = mutate_source_str(
        indoc! {r#"
            fn describe(x: i32) -> &'static str {
                #[mutants::exclude_re("delete match arm")]
                match x {
                    0 => "zero",
                    1 => "one",
                    _ => "other",
                }
            }
        "#},
        &options,
    )
    .unwrap();
    let names: Vec<String> = mutants.iter().map(|m| m.name(false)).collect();

    assert!(
        !names.iter().any(|n| n.contains("delete match arm")),
        "match arm deletion mutants should be filtered out: {names:?}"
    );
}

/// `exclude_re` on a match expression must also suppress the guard-replacement
/// mutants generated for its arm guards.
#[test]
fn exclude_re_attr_on_match_expr_filters_guard_replacements() {
    let options = Options::default();
    let mutants = mutate_source_str(
        indoc! {r#"
            fn pick(x: i32, y: i32) -> &'static str {
                #[mutants::exclude_re("replace match guard")]
                match x {
                    n if n > y => "gt",
                    _ => "le",
                }
            }
        "#},
        &options,
    )
    .unwrap();
    let names: Vec<String> = mutants.iter().map(|m| m.name(false)).collect();

    assert!(
        !names.iter().any(|n| n.contains("replace match guard")),
        "match guard mutants should be filtered out: {names:?}"
    );
}

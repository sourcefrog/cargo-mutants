//! Cross-cutting tests for expression-level `#[mutants::exclude_re(...)]`:
//! interactions with outer scopes and with the higher-precedence
//! `#[mutants::skip]` attribute.

use indoc::indoc;
use test_log::test;

use crate::Options;
use crate::visit::mutate_source_str;

/// An expression-level `exclude_re` must inherit from any outer-scope
/// `exclude_re` (function-level, mod-level, file-level), and additionally
/// suppress its own pattern. Patterns from both scopes combine: a mutant
/// matching either is excluded.
#[test]
fn exclude_re_expr_inherits_outer_scope_patterns() {
    let options = Options::default();
    let mutants = mutate_source_str(
        indoc! {r#"
            #![mutants::exclude_re("replace match guard")]

            fn pick(x: i32, y: i32) -> &'static str {
                #[mutants::exclude_re("delete match arm")]
                match x {
                    0 => "zero",
                    n if n > y => "gt",
                    _ => "other",
                }
            }
        "#},
        &options,
    )
    .unwrap();
    let names: Vec<String> = mutants.iter().map(|m| m.name(false)).collect();

    // Inner expression-level pattern suppresses arm deletions.
    assert!(
        !names.iter().any(|n| n.contains("delete match arm")),
        "inner expression exclude_re should suppress arm deletions: {names:?}"
    );
    // Outer file-level pattern still suppresses guard mutants on the same match.
    assert!(
        !names.iter().any(|n| n.contains("replace match guard")),
        "outer file exclude_re should suppress guard mutants: {names:?}"
    );
}

/// A `#[mutants::skip]` on an expression must take precedence over a
/// sibling `#[mutants::exclude_re(...)]` even when the regex is invalid:
/// the skip short-circuits and no error is produced. This locks in the
/// "skip wins" precedence semantics that future unifying refactors must
/// preserve.
#[test]
fn skip_attr_wins_over_invalid_exclude_re_on_same_expression() {
    let options = Options::default();
    let result = mutate_source_str(
        indoc! {r#"
            fn helper(_x: i32) {}

            fn driver(a: i32, b: i32) {
                #[mutants::skip]
                #[mutants::exclude_re("(unclosed")]
                helper(a + b);
            }
        "#},
        &options,
    );
    // `skip` short-circuits before `exclude_re` is parsed, so the invalid
    // regex is never evaluated and no error is raised.
    let mutants = result.expect("skip should short-circuit before exclude_re is parsed");
    let names: Vec<String> = mutants.iter().map(|m| m.name(false)).collect();
    // The binary `+` inside the skipped call should produce no mutants.
    assert!(
        !names.iter().any(|n| n.contains("replace + with")),
        "skip should suppress all nested mutants in the call: {names:?}"
    );
}

/// As above but with the attribute order reversed, to guarantee the
/// precedence is order-independent.
#[test]
fn skip_attr_wins_over_invalid_exclude_re_on_same_expression_reverse_order() {
    let options = Options::default();
    let result = mutate_source_str(
        indoc! {r#"
            fn helper(_x: i32) {}

            fn driver(a: i32, b: i32) {
                #[mutants::exclude_re("(unclosed")]
                #[mutants::skip]
                helper(a + b);
            }
        "#},
        &options,
    );
    let mutants = result.expect("skip should short-circuit before exclude_re is parsed");
    let names: Vec<String> = mutants.iter().map(|m| m.name(false)).collect();
    assert!(
        !names.iter().any(|n| n.contains("replace + with")),
        "skip should suppress nested mutants regardless of attr order: {names:?}"
    );
}

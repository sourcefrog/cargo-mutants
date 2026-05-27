//! Tests for expression-level `#[mutants::exclude_re(...)]` on unary
//! operator expressions.

use indoc::indoc;
use test_log::test;

use crate::Options;
use crate::visit::mutate_source_str;

/// An expression-level `exclude_re` on a unary operator expression must
/// suppress the unary mutant, while leaving sibling expressions in the
/// same function alone.
#[test]
fn exclude_re_attr_on_unary_expr_filters_unary_mutant() {
    let options = Options::default();
    let mutants = mutate_source_str(
        indoc! {r#"
            fn invert(b: bool) -> bool {
                let _ = #[mutants::exclude_re("delete !")] !b;
                !b
            }
        "#},
        &options,
    )
    .unwrap();
    let names: Vec<String> = mutants.iter().map(|m| m.name(false)).collect();

    // We should still see one `delete !` mutant — the second `!b` is not
    // covered by the attribute. The attribute only suppresses the `!` on
    // the annotated expression, not both.
    let delete_bang_count = names.iter().filter(|n| n.contains("delete !")).count();
    assert_eq!(
        delete_bang_count, 1,
        "exactly one `delete !` mutant should remain (only the unannotated one): {names:?}"
    );
}

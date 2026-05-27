//! Tests that `#[mutants::skip]` on a unary operator expression suppresses
//! the unary mutant, while a sibling unary in the same function remains.

use indoc::indoc;
use test_log::test;

use crate::Options;
use crate::visit::mutate_source_str;

#[test]
fn skip_attr_on_unary_expression_suppresses_only_that_unary_mutant() {
    let mutants = mutate_source_str(
        indoc! {r#"
            fn invert(b: bool) -> bool {
                let _ = #[mutants::skip] !b;
                !b
            }
        "#},
        &Options::default(),
    )
    .unwrap();
    let names: Vec<String> = mutants.iter().map(|m| m.name(false)).collect();

    let delete_bang_count = names.iter().filter(|n| n.contains("delete !")).count();
    assert_eq!(
        delete_bang_count, 1,
        "exactly one `delete !` mutant should remain (only the unannotated one): {names:?}"
    );
}

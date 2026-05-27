//! Tests that `#[mutants::skip]` on a match expression suppresses both
//! arm-deletion and guard-replacement mutants for that match.

use indoc::indoc;
use test_log::test;

use crate::Options;
use crate::visit::mutate_source_str;

#[test]
fn skip_attr_on_match_expression_suppresses_arm_and_guard_mutants() {
    let mutants = mutate_source_str(
        indoc! {r#"
            fn pick(x: i32, y: i32) -> &'static str {
                #[mutants::skip]
                match x {
                    0 => "zero",
                    n if n > y => "gt",
                    _ => "other",
                }
            }
        "#},
        &Options::default(),
    )
    .unwrap();
    let names: Vec<String> = mutants.iter().map(|m| m.name(false)).collect();

    assert!(
        !names.iter().any(|n| n.contains("delete match arm")),
        "match arm deletion mutants should be suppressed: {names:?}"
    );
    assert!(
        !names.iter().any(|n| n.contains("replace match guard")),
        "match guard mutants should be suppressed: {names:?}"
    );
}

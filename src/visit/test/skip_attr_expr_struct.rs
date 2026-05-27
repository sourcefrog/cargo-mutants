//! Tests that `#[mutants::skip]` on a struct literal expression suppresses
//! the field-deletion mutants generated for that literal.

use indoc::indoc;
use test_log::test;

use crate::Options;
use crate::visit::mutate_source_str;

#[test]
fn skip_attr_on_struct_literal_expression_suppresses_field_deletions() {
    let mutants = mutate_source_str(
        indoc! {r#"
            #[derive(Default)]
            struct Settings {
                enabled: bool,
                count: i32,
            }

            fn make() -> Settings {
                #[mutants::skip]
                Settings {
                    enabled: true,
                    count: 1,
                    ..Default::default()
                }
            }
        "#},
        &Options::default(),
    )
    .unwrap();
    let names: Vec<String> = mutants.iter().map(|m| m.name(false)).collect();

    assert!(
        !names.iter().any(|n| n.contains("delete field")),
        "delete field mutants should be suppressed for skipped struct literal: {names:?}"
    );
}

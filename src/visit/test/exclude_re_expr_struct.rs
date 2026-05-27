//! Tests for expression-level `#[mutants::exclude_re(...)]` on struct
//! literal expressions.

use indoc::indoc;
use test_log::test;

use crate::Options;
use crate::visit::mutate_source_str;

/// `exclude_re` on a struct literal expression with `..Default::default()`
/// must suppress the field-deletion mutants for that struct literal.
#[test]
fn exclude_re_attr_on_struct_expr_filters_field_deletions() {
    let options = Options::default();
    let mutants = mutate_source_str(
        indoc! {r#"
            #[derive(Default)]
            struct Settings {
                enabled: bool,
                count: i32,
            }

            fn make() -> Settings {
                #[mutants::exclude_re("delete field enabled")]
                Settings {
                    enabled: true,
                    count: 1,
                    ..Default::default()
                }
            }
        "#},
        &options,
    )
    .unwrap();
    let names: Vec<String> = mutants.iter().map(|m| m.name(false)).collect();

    // The targeted field is filtered out…
    assert!(
        !names.iter().any(|n| n.contains("delete field enabled")),
        "delete `enabled` field mutant should be filtered: {names:?}"
    );
    // …but the other field's mutant remains.
    assert!(
        names.iter().any(|n| n.contains("delete field count")),
        "delete `count` field mutant should remain: {names:?}"
    );
}

//! Tests that `#![mutants::skip]` as a file-level inner attribute
//! suppresses every mutant in the file.

use indoc::indoc;
use test_log::test;

use crate::Options;
use crate::visit::mutate_source_str;

#[test]
fn skip_attr_as_file_inner_attribute_suppresses_all_mutants() {
    let mutants = mutate_source_str(
        indoc! {r#"
            #![mutants::skip]

            fn add(a: i32, b: i32) -> i32 {
                a + b
            }

            fn sub(a: i32, b: i32) -> i32 {
                a - b
            }
        "#},
        &Options::default(),
    )
    .unwrap();
    assert!(
        mutants.is_empty(),
        "file inner #![mutants::skip] should suppress all mutants, got: {mutants:?}"
    );
}

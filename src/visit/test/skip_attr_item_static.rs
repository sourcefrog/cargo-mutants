//! Tests that `#[mutants::skip]` on a top-level `static` item suppresses
//! mutants generated from inside its initializer expression.

use indoc::indoc;
use test_log::test;

use crate::Options;
use crate::mutant::Mutant;
use crate::visit::mutate_source_str;

#[test]
fn skip_attr_on_item_static_suppresses_initializer_mutants() {
    // Different operators on each static so the resulting mutants can be
    // attributed unambiguously to their source item via `original_text()`,
    // independently of line numbers or whitespace.
    let mutants = mutate_source_str(
        indoc! {r#"
            #[mutants::skip]
            pub static SKIPPED_FLAGS: u32 = 0b0001 ^ 0b0010;

            pub static OTHER_FLAGS: u32 = 0b0100 | 0b1000;
        "#},
        &Options::default(),
    )
    .unwrap();
    let originals: Vec<String> = mutants.iter().map(Mutant::original_text).collect();

    assert!(
        !originals.iter().any(|o| o == "^"),
        "operators inside a skipped static initializer should produce no mutants: {mutants:?}"
    );
    assert!(
        originals.iter().any(|o| o == "|"),
        "sibling unskipped static should still produce mutants: {mutants:?}"
    );
}

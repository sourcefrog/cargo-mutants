//! Tests that `#[mutants::skip]` on an associated `const` inside an `impl`
//! block suppresses mutants generated from inside its initializer expression.

use indoc::indoc;
use test_log::test;

use crate::Options;
use crate::mutant::Mutant;
use crate::visit::mutate_source_str;

#[test]
fn skip_attr_on_impl_associated_const_suppresses_initializer_mutants() {
    // Different operators on each associated const so the resulting mutants
    // can be attributed unambiguously to their source item via
    // `original_text()`, independently of line numbers or whitespace.
    let mutants = mutate_source_str(
        indoc! {r#"
            pub struct S;

            impl S {
                #[mutants::skip]
                pub const SKIPPED_FLAGS: u32 = 0b0001 ^ 0b0010;

                pub const OTHER_FLAGS: u32 = 0b0100 | 0b1000;
            }
        "#},
        &Options::default(),
    )
    .unwrap();
    let originals: Vec<String> = mutants.iter().map(Mutant::original_text).collect();

    assert!(
        !originals.iter().any(|o| o == "^"),
        "operators inside a skipped associated const should produce no mutants: {mutants:?}"
    );
    assert!(
        originals.iter().any(|o| o == "|"),
        "sibling unskipped associated const should still produce mutants: {mutants:?}"
    );
}

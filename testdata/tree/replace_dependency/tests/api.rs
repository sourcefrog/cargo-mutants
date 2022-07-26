use cargo_mutants_testdata_replace_dependency::*;

#[test]
fn zero_is_even() {
    assert_eq!(is_even(0), true);
}

#[test]
fn three_is_not_even() {
    assert_eq!(is_even(3), false);
}

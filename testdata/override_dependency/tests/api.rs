use cargo_mutants_testdata_override_dependency::*;

#[test]
fn zero_is_even() {
    assert_eq!(is_even(0), true);
}

#[test]
fn three_is_not_even() {
    assert_eq!(is_even(3), false);
}

#[test]
fn two_is_even() {
    assert!(is_even(2));
}

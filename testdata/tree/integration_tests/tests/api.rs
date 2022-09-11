use cargo_mutants_testdata_integration_tests::double;

#[test]
fn double_zero() {
    assert_eq!(0, double(0));
}

#[test]
fn double_one() {
    assert_eq!(2, double(1));
}

#[test]
fn double_a_number() {
    let n = a_number();
    assert_eq!(double(n), n + n);
}

/// Example of a non-test function within the test module.
///
/// If mutated to return 0 this will make tests falsely pass.
///
/// Functions in test modules should not be mutated.
fn a_number() -> u32 {
    42
}

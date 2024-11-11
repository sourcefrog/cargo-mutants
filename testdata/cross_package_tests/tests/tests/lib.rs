use cargo_mutants_testdata_cross_package_tests_lib::add;

#[test]
fn test_add() {
    assert_eq!(add(1, 2), 3);
}

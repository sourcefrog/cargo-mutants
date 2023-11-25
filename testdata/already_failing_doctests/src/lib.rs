/// Here is a function. It has a doctest, but the doctest does not even build.
///
/// cargo-mutants tests use this to check that doctests can be skipped.
///
/// ```
/// # use mutants_testdata_already_failing_doctests::takes_one_arg;
/// takes_one_arg(123,123,123);
/// ```
pub fn takes_one_arg(a: usize) -> usize {
    a + 1
}

mod test {
    #[test]
    fn takes_one_arg() {
        assert_eq!(super::takes_one_arg(1), 2);
    }
}

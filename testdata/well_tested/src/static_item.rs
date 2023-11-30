static SHOULD_BE_TRUE: bool = 3 == (2 + 1);

mod test {
    #[test]
    fn static_expression_evaluated() {
        assert!(super::SHOULD_BE_TRUE);
    }
}

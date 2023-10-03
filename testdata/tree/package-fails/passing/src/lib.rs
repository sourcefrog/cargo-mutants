pub fn triple(a: usize) -> usize {
    a * 3
}

#[test]
fn triple_2() {
    assert_eq!(triple(2), 6);
}

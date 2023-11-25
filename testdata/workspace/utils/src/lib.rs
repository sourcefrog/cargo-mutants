pub fn triple(a: i32) -> i32 {
    a * 3
}

#[test]
fn test_triple() {
    assert_eq!(triple(3), 9);
}

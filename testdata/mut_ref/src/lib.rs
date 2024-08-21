pub fn returns_mut_ref(a: &mut Vec<u32>) -> &mut u32 {
    a.get_mut(0).unwrap()
}

#[test]
fn test_mut_ref() {
    let mut a = vec![1, 2, 3];
    *returns_mut_ref(&mut a) += 10;
    assert_eq!(a, [11, 2, 3]);
}

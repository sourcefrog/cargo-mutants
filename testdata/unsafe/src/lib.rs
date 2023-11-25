pub unsafe fn unsafe_fn() -> usize {
    42
}

#[test]
fn test_unsafe_fn() {
    unsafe {
        assert_eq!(unsafe_fn(), 42);
    }
}

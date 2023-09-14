fn is_symlink(unix_mode: u32) -> bool {
    unix_mode & 0o140000 != 0
}

#[test]
fn test_symlink() {
    assert!(is_symlink(0o147777));
}

pub fn is_symlink(unix_permissions: u32) -> bool {
    unix_permissions & 0o140000 != 0
}

#[test]
fn test_symlink_from_known_unix_permissions() {
    assert!(is_symlink(0o147777));
}

#[cfg(unix)]
#[test]
fn test_symlink_on_real_symlink_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let td = tempfile::TempDir::new().unwrap();
    let p = td.path().join("link");
    std::os::unix::fs::symlink("target", &p).unwrap();
    let meta = std::fs::symlink_metadata(&p).unwrap();
    assert!(is_symlink(meta.permissions().mode()));
}

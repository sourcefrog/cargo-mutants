pub fn is_symlink(unix_permissions: u32) -> bool {
    unix_permissions & 0o170_000 == 0o120_000
}

#[test]
fn test_symlink_from_known_unix_permissions() {
    assert!(is_symlink(0o120777));
}

#[cfg(unix)]
#[test]
fn test_symlink_on_real_symlink_permissions() {
    use std::fs::symlink_metadata;
    use std::os::unix::fs::PermissionsExt;

    let td = tempfile::TempDir::new().unwrap();

    let p = td.path().join("link");
    std::os::unix::fs::symlink("target", &p).unwrap();
    let meta = symlink_metadata(&p).unwrap();
    assert!(is_symlink(meta.permissions().mode()));

    assert!(!is_symlink(
        symlink_metadata(td.path()).unwrap().permissions().mode()
    ));

    let file_path = td.path().join("file");
    std::fs::File::create(&file_path).unwrap();
    assert!(!is_symlink(
        symlink_metadata(&file_path).unwrap().permissions().mode()
    ));
}

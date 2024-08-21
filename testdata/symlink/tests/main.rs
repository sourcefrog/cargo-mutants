use std::fs::{read_link, read_to_string};
use std::path::Path;

use cargo_mutants_testdata_symlink::read_through_symlink;

#[test]
fn read_through_symlink_test() {
    assert_eq!(read_through_symlink().trim(), "Hello, world!");
}

/// This should fail from the baseline test if the symlink is somehow
/// missing.
#[test]
fn symlink_testdata_exists() {
    let target = Path::new("testdata/target");
    let symlink = Path::new("testdata/symlink");
    assert!(symlink.is_symlink());
    assert!(target.is_file());
    assert_eq!(read_link(&symlink).unwrap(), Path::new("target"));
    assert_eq!(read_to_string(&target).unwrap().trim(), "Hello, world!");
    assert_eq!(read_to_string(&symlink).unwrap().trim(), "Hello, world!");
}

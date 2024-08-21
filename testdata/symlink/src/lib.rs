use std::path::Path;

pub fn read_through_symlink() -> String {
    let path = Path::new("testdata/symlink");
    assert!(path.is_symlink());
    std::fs::read_to_string(path).unwrap()
}

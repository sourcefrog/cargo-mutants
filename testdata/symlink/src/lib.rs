use std::path::Path;

fn read_through_symlink() -> String {
    let path = Path::new("testdata/symlink");
    assert!(path.is_symlink());
    std::fs::read_to_string(path).unwrap()
}

#[cfg(test)]
mod test {
    #[test]
    fn read_through_symlink_test() {
        assert_eq!(super::read_through_symlink().trim(), "Hello, world!");
    }
}

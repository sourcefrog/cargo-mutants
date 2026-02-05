// Copyright 2022-2025 Martin Pool.

//! Utilities for file paths.

use std::cmp::max;
use std::path::Path;

/// Measures how far above its starting point a path ascends.
///
/// If in following this path you would ever ascend above the starting point,
/// this returns a positive number indicating the number of steps above the
/// starting point.
///
/// This only considers the textual content of the path, and does not look at
/// symlinks or whether the directories actually exist.
pub fn ascent(path: &Path) -> isize {
    let mut max_ascent: isize = 0;
    let mut ascent = 0;
    for component in path.components() {
        match component.as_os_str().to_str() {
            Some("..") => {
                ascent += 1;
                max_ascent = max(ascent, max_ascent);
            }
            Some(".") => (),
            _ => ascent -= 1,
        }
    }
    max_ascent
}

/// An extension trait that helps `Path` print with forward slashes,
/// even on Windows.
///
/// This makes the output more consistent across platforms and so easier
/// to test.
pub trait PathSlashes {
    fn to_slash_path(&self) -> String;
}

impl PathSlashes for Path {
    fn to_slash_path(&self) -> String {
        self.components()
            .filter_map(|c| c.as_os_str().to_str())
            .filter(|c| !c.is_empty())
            .map(|c| if c == "/" || c == "\\" { "" } else { c })
            .collect::<Vec<_>>()
            .join("/")
    }
}

#[cfg(test)]
mod test {
    use std::path::{Path, PathBuf};

    use super::{PathSlashes, ascent};

    #[test]
    fn path_slashes_drops_empty_parts() {
        let mut path = PathBuf::from("/a/b/c/");
        path.push("d/e/f");
        assert_eq!(path.to_slash_path(), "/a/b/c/d/e/f");
    }

    #[test]
    fn path_ascent() {
        assert_eq!(ascent(Path::new(".")), 0);
        assert_eq!(ascent(Path::new("..")), 1);
        assert_eq!(ascent(Path::new("./..")), 1);
        assert_eq!(ascent(Path::new("sub/dir")), 0);
        assert_eq!(ascent(Path::new("sub/dir/../..")), 0);
        assert_eq!(ascent(Path::new("sub/../sub/./..")), 0);
        assert_eq!(ascent(Path::new("../back")), 1);
        assert_eq!(ascent(Path::new("../back/../back")), 1);
        assert_eq!(ascent(Path::new("../back/../../back/down")), 2);
    }
}

// Copyright 2022 Martin Pool.

//! Utilities for file paths.

use std::convert::TryInto;
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use camino::{Utf8Path, Utf8PathBuf};
use serde::Serialize;

/// Measures how far above its starting point a path ascends.
///
/// If in following this path you would ever ascend above the starting point,
/// this returns a whole number indicating the number of steps above the
/// starting point.
///
/// This only considers the textual content of the path, and does not look at
/// symlinks or whether the directories actually exist.
pub fn ascent(path: &Utf8Path) -> isize {
    let mut max_ascent: isize = 0;
    let mut ascent = 0;
    for component in path.components().map(|c| c.as_str()) {
        if component == ".." {
            ascent += 1;
        } else if component != "." {
            ascent -= 1;
        }
        if ascent > max_ascent {
            max_ascent = ascent;
        }
    }
    max_ascent
}

/// An extension trait that helps Utf8Path print with forward slashes,
/// even on Windows.
///
/// This makes the output more consistent across platforms and so easier
/// to test.
pub trait Utf8PathSlashes {
    fn to_slash_path(&self) -> String;
}

impl Utf8PathSlashes for Utf8Path {
    fn to_slash_path(&self) -> String {
        self.components()
            .map(|c| c.as_str())
            .filter(|c| !c.is_empty())
            .map(|c| if c == "/" || c == "\\" { "" } else { c })
            .collect::<Vec<_>>()
            .join("/")
    }
}

/// A path relative to the top of the source tree.
#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord, Serialize)]
pub struct TreeRelativePathBuf(Utf8PathBuf);

impl fmt::Display for TreeRelativePathBuf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self
            .0
            .components()
            .map(|c| c.as_str())
            .collect::<Vec<_>>()
            .join("/");
        f.write_str(&s)
    }
}

impl TreeRelativePathBuf {
    pub fn new(path: Utf8PathBuf) -> Self {
        assert!(path.is_relative());
        TreeRelativePathBuf(path)
    }

    /// Make an empty tree-relative path, identifying the root.
    #[allow(dead_code)]
    pub fn empty() -> Self {
        TreeRelativePathBuf(Utf8PathBuf::new())
    }

    #[allow(dead_code)]
    pub fn from_absolute(path: &Utf8Path, tree_root: &Utf8Path) -> Self {
        TreeRelativePathBuf(
            path.strip_prefix(tree_root)
                .expect("path is within tree root")
                .to_owned(),
        )
    }

    pub fn within(&self, tree_path: &Utf8Path) -> Utf8PathBuf {
        tree_path.join(&self.0)
    }

    /// Return the tree-relative path of the containing directory.
    ///
    /// Panics if there is no parent, i.e. if self is already the tree root.
    pub fn parent(&self) -> TreeRelativePathBuf {
        self.0
            .parent()
            .expect("TreeRelativePath has no parent")
            .to_owned()
            .into()
    }
}

impl From<&Utf8Path> for TreeRelativePathBuf {
    fn from(path_buf: &Utf8Path) -> Self {
        TreeRelativePathBuf::new(path_buf.to_owned())
    }
}

impl From<Utf8PathBuf> for TreeRelativePathBuf {
    fn from(path_buf: Utf8PathBuf) -> Self {
        TreeRelativePathBuf::new(path_buf)
    }
}

impl From<PathBuf> for TreeRelativePathBuf {
    fn from(path_buf: PathBuf) -> Self {
        TreeRelativePathBuf::new(path_buf.try_into().expect("path must be UTF-8"))
    }
}

impl From<&Path> for TreeRelativePathBuf {
    fn from(path: &Path) -> Self {
        TreeRelativePathBuf::from(path.to_owned())
    }
}

impl FromStr for TreeRelativePathBuf {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(TreeRelativePathBuf::new(s.parse()?))
    }
}

#[cfg(test)]
mod test {
    use camino::{Utf8Path, Utf8PathBuf};

    use super::{ascent, Utf8PathSlashes};

    #[test]
    fn path_slashes_drops_empty_parts() {
        let mut path = Utf8PathBuf::from("/a/b/c/");
        path.push("d/e/f");
        assert_eq!(path.to_slash_path(), "/a/b/c/d/e/f");
    }

    #[test]
    fn path_ascent() {
        assert_eq!(ascent(Utf8Path::new(".")), 0);
        assert_eq!(ascent(Utf8Path::new("..")), 1);
        assert_eq!(ascent(Utf8Path::new("sub/dir")), 0);
        assert_eq!(ascent(Utf8Path::new("sub/dir/../..")), 0);
        assert_eq!(ascent(Utf8Path::new("sub/../sub/./..")), 0);
        assert_eq!(ascent(Utf8Path::new("../back")), 1);
        assert_eq!(ascent(Utf8Path::new("../back/../back")), 1);
        assert_eq!(ascent(Utf8Path::new("../back/../../back/down")), 2);
    }
}

// Copyright 2021-2023 Martin Pool

//! Access to a Rust source tree and files.

use std::sync::Arc;

use anyhow::{Context, Result};
use camino::Utf8PathBuf;
#[allow(unused_imports)]
use tracing::{debug, info, warn};

use crate::path::TreeRelativePathBuf;

/// A Rust source file within a source tree.
///
/// It can be viewed either relative to the source tree (for display)
/// or as a path that can be opened (relative to cwd or absolute.)
///
/// Code is normalized to Unix line endings as it's read in, and modified
/// files are written with Unix line endings.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct SourceFile {
    pub package: Package,

    /// Path to the root of the tree.
    pub tree_path: Arc<Utf8PathBuf>,

    /// Path relative to the root of the tree.
    pub tree_relative_path: TreeRelativePathBuf,

    /// Full copy of the source.
    pub code: Arc<String>,
}

#[derive(Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct Package {
    pub name: String,
    pub relative_manifest_path: Utf8PathBuf,
    pub version: String,
}

impl SourceFile {
    /// Construct a SourceFile representing a file within a tree.
    ///
    /// This eagerly loads the text of the file.
    pub fn new(
        tree_path: Arc<Utf8PathBuf>,
        tree_relative_path: TreeRelativePathBuf,
        package: Package,
    ) -> Result<SourceFile> {
        let full_path = tree_relative_path.within(&tree_path);
        let code = std::fs::read_to_string(&full_path)
            .with_context(|| format!("failed to read source of {full_path:?}"))?
            .replace("\r\n", "\n");
        Ok(SourceFile {
            tree_path: Arc::clone(&tree_path),
            tree_relative_path,
            code: Arc::new(code),
            package,
        })
    }

    pub fn child_mod(&self, tree_relative_path: TreeRelativePathBuf) -> Result<SourceFile> {
        SourceFile::new(
            Arc::clone(&self.tree_path),
            tree_relative_path,
            self.package.clone(),
        )
    }
    /// Return the path of this file relative to the tree root, with forward slashes.
    pub fn tree_relative_slashes(&self) -> String {
        self.tree_relative_path.to_string()
    }

    /// Return the path of this file relative to the base of the source tree.
    pub fn tree_relative_path(&self) -> &TreeRelativePathBuf {
        &self.tree_relative_path
    }
}

#[cfg(test)]
mod test {
    use std::fs::File;
    use std::io::Write;

    use camino::Utf8Path;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn source_file_normalizes_crlf() {
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_dir_path = Utf8Path::from_path(temp_dir.path()).unwrap();
        let file_name = "lib.rs";
        File::create(temp_dir.path().join(file_name))
            .unwrap()
            .write_all(b"fn main() {\r\n    640 << 10;\r\n}\r\n")
            .unwrap();
        let package = Package {
            name: "foo".into(),
            relative_manifest_path: "Cargo.toml".into(),
            version: "0.0.0".into(),
        };

        let source_file = SourceFile::new(
            Arc::new(temp_dir_path.to_owned()),
            file_name.parse().unwrap(),
            package,
        )
        .unwrap();
        assert_eq!(*source_file.code, "fn main() {\n    640 << 10;\n}\n");
    }
}

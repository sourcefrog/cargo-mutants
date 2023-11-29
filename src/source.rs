// Copyright 2021-2023 Martin Pool

//! Access to a Rust source tree and files.

use std::sync::Arc;

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
#[allow(unused_imports)]
use tracing::{debug, info, warn};

use crate::package::Package;
use crate::path::Utf8PathSlashes;

/// A Rust source file within a source tree.
///
/// It can be viewed either relative to the source tree (for display)
/// or as a path that can be opened (relative to cwd or absolute.)
///
/// Code is normalized to Unix line endings as it's read in, and modified
/// files are written with Unix line endings.
#[derive(Debug, Eq, PartialEq, Hash)]
pub struct SourceFile {
    /// Package within the workspace.
    pub package: Arc<Package>,

    /// Path of this source file relative to workspace.
    pub tree_relative_path: Utf8PathBuf,

    /// Full copy of the source.
    pub code: String,
}

impl SourceFile {
    /// Construct a SourceFile representing a file within a tree.
    ///
    /// This eagerly loads the text of the file.
    pub fn new(
        tree_path: &Utf8Path,
        tree_relative_path: Utf8PathBuf,
        package: &Arc<Package>,
    ) -> Result<SourceFile> {
        let full_path = tree_path.join(&tree_relative_path);
        let code = std::fs::read_to_string(&full_path)
            .with_context(|| format!("failed to read source of {full_path:?}"))?
            .replace("\r\n", "\n");
        Ok(SourceFile {
            tree_relative_path,
            code,
            package: Arc::clone(package),
        })
    }

    /// Return the path of this file relative to the tree root, with forward slashes.
    pub fn tree_relative_slashes(&self) -> String {
        self.tree_relative_path.to_slash_path()
    }

    pub fn path(&self) -> &Utf8Path {
        self.tree_relative_path.as_path()
    }
}

#[cfg(test)]
mod test {
    use std::fs::File;
    use std::io::Write;

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

        let source_file = SourceFile::new(
            temp_dir_path,
            file_name.parse().unwrap(),
            &Arc::new(Package {
                name: "imaginary-package".to_owned(),
                relative_manifest_path: "whatever/Cargo.toml".into(),
            }),
        )
        .unwrap();
        assert_eq!(source_file.code, "fn main() {\n    640 << 10;\n}\n");
    }
}

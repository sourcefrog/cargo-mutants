// Copyright 2021-2025 Martin Pool

//! Access to a Rust source tree and files.

use std::fmt;
use std::fs::read_to_string;
use std::sync::Arc;

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
#[allow(unused_imports)]
use tracing::{debug, info, warn};

use crate::package::Package;
use crate::path::{ascent, Utf8PathSlashes};
use crate::span::LineColumn;

/// A Rust source file within a source tree.
///
/// It can be viewed either relative to the source tree (for display)
/// or as a path that can be opened (relative to cwd or absolute.)
///
/// Code is normalized to Unix line endings as it's read in, and modified
/// files are written with Unix line endings.
#[derive(Clone, Eq, PartialEq)]
#[allow(clippy::module_name_repetitions)]
pub struct SourceFile {
    /// What package includes this file?
    pub package: Arc<Package>,

    /// Path of this source file relative to workspace.
    pub tree_relative_path: Utf8PathBuf,

    /// Full copy of the unmodified source.
    ///
    /// This is held in an [Arc] so that `SourceFile`s can be cloned without using excessive
    /// amounts of memory.
    pub code: Arc<String>,

    /// True if this is the top source file for its target: typically but
    /// not always `lib.rs` or `main.rs`.
    pub is_top: bool,
}

#[allow(clippy::missing_fields_in_debug)] // intentional
impl fmt::Debug for SourceFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceFile")
            .field("package", &self.package)
            .field("tree_relative_path", &self.tree_relative_path)
            // Omit `code`, it can be too big.
            .field("is_top", &self.is_top)
            .finish()
    }
}

impl SourceFile {
    /// Construct a `SourceFile` representing a file within a tree.
    ///
    /// This eagerly loads the text of the file.
    ///
    /// This also skip files outside of the tree, returning `Ok(None)`.
    pub fn load(
        tree_path: &Utf8Path,
        tree_relative_path: &Utf8Path,
        package: &Package,
        is_top: bool,
    ) -> Result<Option<SourceFile>> {
        // TODO: Perhaps the caller should be responsible for checking this?
        if ascent(tree_relative_path) > 0 {
            warn!(
                "skipping source outside of tree: {:?}",
                tree_relative_path.to_slash_path()
            );
            return Ok(None);
        }
        let full_path = tree_path.join(tree_relative_path);
        let code = Arc::new(
            read_to_string(&full_path)
                .with_context(|| format!("failed to read source of {full_path:?}"))?
                .replace("\r\n", "\n"),
        );
        Ok(Some(SourceFile {
            tree_relative_path: tree_relative_path.to_owned(),
            code,
            package: Arc::new(package.clone()),
            is_top,
        }))
    }

    /// Construct from in-memory text.
    #[cfg(test)]
    pub fn for_tests<P: Into<Utf8PathBuf>>(
        tree_relative_path: P,
        code: &str,
        package_name: &str,
        is_top: bool,
    ) -> SourceFile {
        let tree_relative_path = tree_relative_path.into();
        let top_sources = if is_top {
            vec![tree_relative_path.clone()]
        } else {
            vec!["src/lib.rs".into()]
        };
        SourceFile {
            tree_relative_path,
            code: Arc::new(code.to_owned()),
            package: Arc::new(Package {
                name: package_name.to_owned(),
                relative_dir: Utf8PathBuf::new(),
                top_sources,
                version: "0.1.0".to_owned(),
            }),
            is_top,
        }
    }

    /// Return the path of this file relative to the tree root, with forward slashes.
    pub fn tree_relative_slashes(&self) -> String {
        self.tree_relative_path.to_slash_path()
    }

    pub fn path(&self) -> &Utf8Path {
        self.tree_relative_path.as_path()
    }

    pub fn code(&self) -> &str {
        self.code.as_str()
    }

    /// Format a location within this source file for display to the user
    pub fn format_source_location(&self, location: LineColumn) -> String {
        let source_file = self.tree_relative_slashes();
        let LineColumn { line, column } = location;
        format!("{source_file}:{line}:{column}")
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
        let package = Package {
            name: "imaginary-package".to_owned(),
            relative_dir: Utf8PathBuf::from(""),
            top_sources: vec!["src/lib.rs".into()],
            version: "0.1.0".to_owned(),
        };
        let source_file = SourceFile::load(temp_dir_path, Utf8Path::new(file_name), &package, true)
            .unwrap()
            .unwrap();
        assert_eq!(source_file.code(), "fn main() {\n    640 << 10;\n}\n");
    }

    #[test]
    fn skips_files_outside_of_workspace() {
        let package = Arc::new(Package {
            name: "imaginary-package".to_owned(),
            relative_dir: Utf8PathBuf::from(""),
            top_sources: vec!["src/lib.rs".into()],
            version: "0.1.0".to_owned(),
        });
        let source_file = SourceFile::load(
            Utf8Path::new("unimportant"),
            Utf8Path::new("../outside_workspace.rs"),
            &package,
            true,
        )
        .unwrap();
        assert_eq!(source_file, None);
    }
}

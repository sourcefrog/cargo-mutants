// Copyright 2021, 2022 Martin Pool

//! Access to a Rust source tree and files.

use std::rc::Rc;

use anyhow::{Context, Result};
use camino::Utf8Path;
#[allow(unused_imports)]
use tracing::{debug, info, warn};

use crate::path::TreeRelativePathBuf;
use crate::*;

/// A Rust source file within a source tree.
///
/// It can be viewed either relative to the source tree (for display)
/// or as a path that can be opened (relative to cwd or absolute.)
///
/// Code is normalized to Unix line endings as it's read in, and modified
/// files are written with Unix line endings.
#[derive(Clone, PartialEq, Eq)]
pub struct SourceFile {
    /// Path relative to the root of the tree.
    pub tree_relative_path: TreeRelativePathBuf,

    /// Full copy of the source.
    pub code: Rc<String>,

    /// Package within the workspace.
    pub package_name: Rc<String>,
}

impl SourceFile {
    /// Construct a SourceFile representing a file within a tree.
    ///
    /// This eagerly loads the text of the file.
    pub fn new(
        tree_path: &Utf8Path,
        tree_relative_path: TreeRelativePathBuf,
        package_name: Rc<String>,
    ) -> Result<SourceFile> {
        let full_path = tree_relative_path.within(tree_path);
        let code = std::fs::read_to_string(&full_path)
            .with_context(|| format!("failed to read source of {:?}", full_path))?
            .replace("\r\n", "\n");
        Ok(SourceFile {
            tree_relative_path,
            code: Rc::new(code),
            package_name,
        })
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

/// Some kind of source tree.
///
/// The specific type of tree depends on which tool is used to build it.
///
/// It knows its filesystem path, and can provide a list of source files, from
/// which mutations can be generated.
pub trait SourceTree: std::fmt::Debug {
    /// Find mutatable source files in the tree.
    fn source_files(&self, options: &Options) -> Result<Vec<SourceFile>>;

    /// Path of the root of the tree.
    fn path(&self) -> &Utf8Path;

    fn mutants(&self, options: &Options) -> Result<Vec<Mutant>> {
        let mut mutants = Vec::new();
        for sf in self.source_files(options)? {
            check_interrupted()?;
            mutants.extend(discover_mutants(sf.into())?);
        }
        if let Some(examine_names) = &options.examine_names {
            if !examine_names.is_empty() {
                mutants.retain(|m| examine_names.is_match(&m.to_string()));
            }
        }
        if let Some(exclude_names) = &options.exclude_names {
            if !exclude_names.is_empty() {
                mutants.retain(|m| !exclude_names.is_match(&m.to_string()));
            }
        }
        Ok(mutants)
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
            Rc::new("imaginary-package".to_owned()),
        )
        .unwrap();
        assert_eq!(*source_file.code, "fn main() {\n    640 << 10;\n}\n");
    }
}

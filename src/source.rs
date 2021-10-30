// Copyright 2021 Martin Pool

//! Access to a Rust source tree and files.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::{anyhow, Context, Result};
use path_slash::PathExt;
use serde::{Serialize, Serializer};
use syn::visit::Visit;

use crate::mutate::{DiscoveryVisitor, Mutation};

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
    tree_relative: PathBuf,

    /// Full copy of the source.
    pub code: Rc<String>,
}

impl SourceFile {
    /// Construct a SourceFile representing a file within a tree.
    ///
    /// This eagerly loads the text of the file.
    pub fn new(tree_path: &Path, tree_relative: &Path) -> Result<SourceFile> {
        let full_path = tree_path.join(tree_relative);
        let code = std::fs::read_to_string(&full_path)
            .with_context(|| format!("failed to read source of {:?}", full_path))?
            .replace("\r\n", "\n");
        Ok(SourceFile {
            tree_relative: tree_relative.to_owned(),
            code: Rc::new(code),
        })
    }

    /// Return the path of this file relative to the tree root, with forward slashes.
    pub fn tree_relative_slashes(&self) -> String {
        self.tree_relative.to_slash_lossy()
    }

    /// Generate a list of all mutation possibilities within this file.
    pub fn mutations(&self) -> Result<Vec<Mutation>> {
        let syn_file = syn::parse_str::<syn::File>(&self.code)?;
        let mut v = DiscoveryVisitor::new(self);
        v.visit_file(&syn_file);
        Ok(v.mutations)
    }

    /// Return the path of this file relative to a given directory.
    // TODO: Maybe let the caller do this.
    pub fn within_dir(&self, dir: &Path) -> PathBuf {
        dir.join(&self.tree_relative)
    }
}

impl Serialize for SourceFile {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize the path by hand to get forward slashes.
        serializer.serialize_str(&self.tree_relative_slashes())
    }
}

#[derive(Debug)]
pub struct SourceTree {
    root: PathBuf,
}

impl SourceTree {
    pub fn new(root: &Path) -> Result<SourceTree> {
        if !root.join("Cargo.toml").is_file() {
            return Err(anyhow!(
                "{} does not contain a Cargo.toml: specify a crate directory",
                root.to_slash_lossy()
            ));
        }
        Ok(SourceTree {
            root: root.to_owned(),
        })
    }

    /// Return all the mutations that could possibly be applied to this tree.
    pub fn mutations(&self) -> Result<Vec<Mutation>> {
        let mut r = Vec::new();
        for sf in self.source_files() {
            r.extend(Rc::new(sf).mutations()?);
        }
        Ok(r)
    }

    /// Return an iterator of `src/**/*.rs` paths relative to the root.
    pub fn source_files(&self) -> impl Iterator<Item = SourceFile> + '_ {
        walkdir::WalkDir::new(self.root.join("src"))
            .sort_by_file_name()
            .into_iter()
            .filter_map(|r| {
                r.map_err(|err| eprintln!("error walking source tree: {:?}", err))
                    .ok()
            })
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| entry.into_path())
            .filter(|path| {
                path.extension()
                    .map_or(false, |p| p.eq_ignore_ascii_case("rs"))
            })
            .filter_map(move |full_path| {
                let tree_relative = full_path.strip_prefix(&self.root).unwrap();
                SourceFile::new(&self.root, tree_relative)
                    .map_err(|err| {
                        eprintln!(
                            "error reading source {}: {}",
                            full_path.to_slash_lossy(),
                            err
                        );
                    })
                    .ok()
            })
    }

    /// Return the path (possibly relative) to the root of the source tree.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[cfg(test)]
mod test {
    use std::fs::File;
    use std::io::Write;

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn source_files_in_testdata_factorial() {
        let source_paths = SourceTree::new(Path::new("testdata/tree/factorial"))
            .unwrap()
            .source_files()
            .collect::<Vec<SourceFile>>();
        assert_eq!(source_paths.len(), 1);
        assert_eq!(
            source_paths[0].tree_relative,
            PathBuf::from("src/bin/main.rs"),
        );
    }

    #[test]
    fn error_opening_subdirectory_of_crate() {
        let result = SourceTree::new(Path::new("testdata/tree/factorial/src"));
        assert!(result.is_err());
    }

    #[test]
    fn error_opening_outside_of_crate() {
        let result = SourceTree::new(Path::new("/"));
        assert!(result.is_err());
    }

    #[test]
    fn source_file_normalizes_crlf() {
        let temp = tempfile::tempdir().unwrap();
        let file_name = "lib.rs";
        File::create(temp.path().join(file_name))
            .unwrap()
            .write_all(b"fn main() {\r\n    640 << 10;\r\n}\r\n")
            .unwrap();
        let source_file = SourceFile::new(&temp.path(), Path::new(file_name)).unwrap();
        assert_eq!(*source_file.code, "fn main() {\n    640 << 10;\n}\n");
    }
}

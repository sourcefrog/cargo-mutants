// Copyright 2021, 2022 Martin Pool

//! Access to a Rust source tree and files.

use std::collections::BTreeSet;
use std::convert::TryInto;
use std::fmt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str::FromStr;

use anyhow::{anyhow, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use globset::GlobSet;

use crate::*;

/// A path relative to the top of the source tree.
#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
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
    tree_relative_path: TreeRelativePathBuf,

    /// Full copy of the source.
    pub code: Rc<String>,
}

impl SourceFile {
    /// Construct a SourceFile representing a file within a tree.
    ///
    /// This eagerly loads the text of the file.
    pub fn new(
        tree_path: &Utf8Path,
        tree_relative_path: TreeRelativePathBuf,
    ) -> Result<SourceFile> {
        let full_path = tree_relative_path.within(tree_path);
        let code = std::fs::read_to_string(&full_path)
            .with_context(|| format!("failed to read source of {:?}", full_path))?
            .replace("\r\n", "\n");
        Ok(SourceFile {
            tree_relative_path,
            code: Rc::new(code),
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

/// An original Rust source tree.
///
/// This is never written to, only examined and used as a source for copying to
/// build dirs.
#[derive(Debug)]
pub struct SourceTree {
    root: Utf8PathBuf,
    metadata: cargo_metadata::Metadata,
}

impl SourceTree {
    /// Open a source tree.
    ///
    /// This eagerly loads cargo metadata from the enclosed `Cargo.toml`, so the
    /// tree must be minimally valid Rust.
    pub fn new(path: &Utf8Path) -> Result<SourceTree> {
        let cargo_toml_path = path.join("Cargo.toml");
        if !cargo_toml_path.is_file() {
            return Err(anyhow!(
                "{} does not contain a Cargo.toml: specify a crate directory",
                path.to_slash_path()
            ));
        }
        let metadata = cargo_metadata::MetadataCommand::new()
            .manifest_path(&cargo_toml_path)
            .exec()
            .context("run cargo metadata")?;
        Ok(SourceTree {
            root: path.to_owned(),
            metadata,
        })
    }

    /// Return all the mutations that could possibly be applied to this tree.
    pub fn mutants(&self, options: &Options) -> Result<Vec<Mutant>> {
        let mut r = Vec::new();
        for sf in self.source_files(options)? {
            check_interrupted()?;
            r.extend(discover_mutants(sf.into())?);
        }
        Ok(r)
    }

    /// Return an iterator of `src/**/*.rs` paths relative to the root.
    pub fn source_paths(
        &self,
        options: &Options,
    ) -> Result<impl IntoIterator<Item = TreeRelativePathBuf>> {
        let top_sources = cargo_metadata_sources(&self.metadata)?;
        indirect_sources(&self.root, top_sources, &options.globset)
    }

    /// Return an iterator of [SourceFile] object, eagerly loading their content.
    pub fn source_files(&self, options: &Options) -> Result<impl Iterator<Item = SourceFile> + '_> {
        // TODO: Maybe don't eagerly read them here...?
        let source_paths = self.source_paths(options)?;
        let root = self.root.clone();
        Ok(source_paths.into_iter().filter_map(move |trp| {
            SourceFile::new(&root, trp.clone())
                .map_err(|err| {
                    eprintln!("error reading source {}: {}", trp, err);
                })
                .ok()
        }))
    }

    /// Return the path (possibly relative) to the root of the source tree.
    pub fn path(&self) -> &Utf8Path {
        &self.root
    }

    /// Return the name of the root crate, as an identifier for this tree.
    pub fn root_crate_name(&self) -> Result<String> {
        todo!()
    }
}

fn indirect_sources(
    root_dir: &Utf8Path,
    top_sources: impl IntoIterator<Item = TreeRelativePathBuf>,
    globset: &Option<GlobSet>,
) -> Result<BTreeSet<TreeRelativePathBuf>> {
    let dirs: BTreeSet<TreeRelativePathBuf> = top_sources.into_iter().map(|p| p.parent()).collect();
    let mut files: BTreeSet<TreeRelativePathBuf> = BTreeSet::new();
    for top_dir in dirs {
        for p in walkdir::WalkDir::new(top_dir.within(root_dir))
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
            .map(move |full_path| {
                full_path
                    .strip_prefix(&root_dir)
                    .expect("strip prefix")
                    .to_owned()
            })
            .filter(|rel_path| globset.as_ref().map_or(true, |gs| gs.is_match(rel_path)))
        {
            files.insert(p.into());
        }
    }
    Ok(files)
}

/// Given a path to a cargo manifest, find all the directly-referenced source files.
fn cargo_metadata_sources(
    metadata: &cargo_metadata::Metadata,
) -> Result<BTreeSet<TreeRelativePathBuf>> {
    let mut found = BTreeSet::new();
    if let Some(pkg) = metadata.root_package() {
        let pkg_dir = pkg.manifest_path.parent().unwrap();
        for target in &pkg.targets {
            if target.kind == ["lib"] || target.kind == ["bin"] {
                if let Ok(relpath) = target.src_path.strip_prefix(&pkg_dir) {
                    let relpath = TreeRelativePathBuf::new(relpath.into());
                    found.insert(relpath);
                } else {
                    eprintln!("{:?} is not in {:?}", target.src_path, pkg_dir);
                }
            }
        }
    }
    Ok(found)
}

#[cfg(test)]
mod test {
    use std::fs::File;
    use std::io::Write;

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn source_files_in_testdata_factorial() {
        let source_paths = SourceTree::new(Utf8Path::new("testdata/tree/factorial"))
            .unwrap()
            .source_files(&Options::default())
            .unwrap()
            .collect::<Vec<SourceFile>>();
        assert_eq!(source_paths.len(), 1);
        assert_eq!(
            source_paths[0].tree_relative_path().to_string(),
            "src/bin/main.rs",
        );
    }

    #[test]
    fn error_opening_subdirectory_of_crate() {
        let result = SourceTree::new(Utf8Path::new("testdata/tree/factorial/src"));
        assert!(result.is_err());
    }

    #[test]
    fn error_opening_outside_of_crate() {
        let result = SourceTree::new(Utf8Path::new("/"));
        assert!(result.is_err());
    }

    #[test]
    fn source_file_normalizes_crlf() {
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_dir_path = Utf8Path::from_path(temp_dir.path()).unwrap();
        let file_name = "lib.rs";
        File::create(temp_dir.path().join(file_name))
            .unwrap()
            .write_all(b"fn main() {\r\n    640 << 10;\r\n}\r\n")
            .unwrap();

        let source_file = SourceFile::new(temp_dir_path, file_name.parse().unwrap()).unwrap();
        assert_eq!(*source_file.code, "fn main() {\n    640 << 10;\n}\n");
    }
}

// Copyright 2021, 2022 Martin Pool

//! Access to a Rust source tree and files.

use std::collections::BTreeSet;
use std::rc::Rc;

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use globset::GlobSet;
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

/// An original Rust source tree.
///
/// This is never written to, only examined and used as a source for copying to
/// build dirs.
#[derive(Debug)]
pub struct SourceTree {
    /// Root of the source tree, absolute or relative to the cargo-mutants cwd.
    root: Utf8PathBuf,
    metadata: cargo_metadata::Metadata,
}

impl SourceTree {
    /// Open a source tree.
    ///
    /// This eagerly loads cargo metadata from the enclosed `Cargo.toml`.
    ///
    /// `path` may be any path pointing within the tree, including a relative
    /// path.
    ///
    /// The root of the tree is discovered by asking Cargo to walk up and find
    /// the enclosing workspace.
    pub fn new(path: &Utf8Path) -> Result<SourceTree> {
        let cargo_toml_path = cargo::locate_project(path)?;
        info!("cargo_toml_path = {cargo_toml_path}");
        let root = cargo_toml_path
            .parent()
            .expect("Cargo.toml path has no directory?")
            .to_owned();
        let metadata = cargo_metadata::MetadataCommand::new()
            .manifest_path(&cargo_toml_path)
            .exec()
            .context("run cargo metadata")?;
        Ok(SourceTree { metadata, root })
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

    /// Return an iterator of [SourceFile] objects representing all source files
    /// in all packages in the tree, eagerly loading their content.
    pub fn source_files(&self, options: &Options) -> Result<Vec<SourceFile>> {
        let mut r = Vec::new();
        for package_metadata in &self.metadata.workspace_packages() {
            debug!("walk package {:?}", package_metadata.manifest_path);
            let top_sources = direct_package_sources(&self.root, package_metadata)?;
            let source_paths = indirect_source_paths(
                &self.root,
                top_sources,
                &options.examine_globset,
                &options.exclude_globset,
            )?;
            let package_name = Rc::new(package_metadata.name.to_string());
            for source_path in source_paths {
                check_interrupted()?;
                r.push(SourceFile::new(
                    &self.root,
                    source_path,
                    Rc::clone(&package_name),
                )?);
            }
        }
        Ok(r)
    }

    /// Return the path (possibly relative) to the root of the source tree.
    pub fn path(&self) -> &Utf8Path {
        &self.root
    }
}

/// Find all the `.rs` files, by starting from the sources identified by the manifest
/// and walking down.
///
/// This just walks the directory tree rather than following `mod` statements (for now)
/// so it may pick up some files that are not actually linked in.
fn indirect_source_paths(
    root_dir: &Utf8Path,
    top_sources: impl IntoIterator<Item = TreeRelativePathBuf>,
    examine_globset: &Option<GlobSet>,
    exclude_globset: &Option<GlobSet>,
) -> Result<BTreeSet<TreeRelativePathBuf>> {
    let dirs: BTreeSet<TreeRelativePathBuf> = top_sources.into_iter().map(|p| p.parent()).collect();
    let mut files: BTreeSet<TreeRelativePathBuf> = BTreeSet::new();
    for top_dir in dirs {
        for p in walkdir::WalkDir::new(top_dir.within(root_dir))
            .sort_by_file_name()
            .into_iter()
        {
            let p = p.with_context(|| "error walking source tree {top_dir}")?;
            if !p.file_type().is_file() {
                continue;
            }
            let path = p.into_path();
            if !path
                .extension()
                .map_or(false, |p| p.eq_ignore_ascii_case("rs"))
            {
                continue;
            }
            let relative_path = path
                .strip_prefix(&root_dir)
                .expect("strip prefix")
                .to_owned();
            if let Some(examine_globset) = examine_globset {
                if !examine_globset.is_match(&relative_path) {
                    continue;
                }
            }
            if let Some(exclude_globset) = exclude_globset {
                if exclude_globset.is_match(&relative_path) {
                    continue;
                }
            }
            files.insert(relative_path.into());
        }
    }
    Ok(files)
}

/// Find all the files that are named in the `path` of targets in a Cargo manifest that should be tested.
///
/// These are the starting points for discovering source files.
fn direct_package_sources(
    workspace_root: &Utf8Path,
    package_metadata: &cargo_metadata::Package,
) -> Result<Vec<TreeRelativePathBuf>> {
    let mut found = Vec::new();
    let pkg_dir = package_metadata.manifest_path.parent().unwrap();
    for target in &package_metadata.targets {
        if should_mutate_target(target) {
            if let Ok(relpath) = target.src_path.strip_prefix(&workspace_root) {
                let relpath = TreeRelativePathBuf::new(relpath.into());
                found.push(relpath);
            } else {
                let message = format!("{:?} is not in {:?}", target.src_path, pkg_dir);
                eprintln!("{}", message);
                warn!("{}", message);
            }
        } else {
            debug!(
                "skipping target {:?} of kinds {:?}",
                target.name, target.kind
            );
        }
    }
    found.sort();
    found.dedup();
    Ok(found)
}

fn should_mutate_target(target: &cargo_metadata::Target) -> bool {
    target
        .kind
        .iter()
        .any(|k| k.ends_with("lib") || k == "bin" || k == "test")
}

#[cfg(test)]
mod test {
    use std::ffi::OsStr;
    use std::fs::File;
    use std::io::Write;

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn source_files_in_testdata_factorial() {
        let source_paths = SourceTree::new(Utf8Path::new("testdata/tree/factorial"))
            .unwrap()
            .source_files(&Options::default())
            .unwrap();
        assert_eq!(source_paths.len(), 1);
        assert_eq!(
            source_paths[0].tree_relative_path().to_string(),
            "src/bin/factorial.rs",
        );
    }

    #[test]
    fn open_subdirectory_of_crate_opens_the_crate() {
        let source_tree = SourceTree::new(Utf8Path::new("testdata/tree/factorial/src"))
            .expect("open source tree from subdirectory");
        let path = source_tree.path();
        assert!(path.is_dir());
        assert!(path.join("Cargo.toml").is_file());
        assert!(path.join("src/bin/factorial.rs").is_file());
        assert_eq!(path.file_name().unwrap(), OsStr::new("factorial"));
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

        let source_file = SourceFile::new(
            temp_dir_path,
            file_name.parse().unwrap(),
            Rc::new("imaginary-package".to_owned()),
        )
        .unwrap();
        assert_eq!(*source_file.code, "fn main() {\n    640 << 10;\n}\n");
    }
}

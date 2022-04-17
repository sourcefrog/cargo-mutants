// Copyright 2021, 2022 Martin Pool

//! Access to a Rust source tree and files.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::{anyhow, Context, Result};
use globset::GlobSet;
use path_slash::PathExt;

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
    tree_relative: PathBuf,

    /// Full copy of the source.
    pub code: Rc<String>,
}

impl SourceFile {
    /// Construct a SourceFile representing a file within a tree.
    ///
    /// This eagerly loads the text of the file.
    pub fn new(tree_path: &Path, tree_relative: &Path) -> Result<SourceFile> {
        assert!(tree_relative.is_relative());
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

    /// Return the path of this file relative to the base of the source tree.
    pub fn tree_relative_path(&self) -> &Path {
        &self.tree_relative
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
    pub fn mutants(&self, options: &Options) -> Result<Vec<Mutant>> {
        let mut r = Vec::new();
        for sf in self.source_files(options) {
            check_interrupted()?;
            r.extend(discover_mutants(sf.into())?);
        }
        Ok(r)
    }

    /// Return an iterator of `src/**/*.rs` paths relative to the root.
    pub fn source_files(&self, options: &Options) -> impl Iterator<Item = SourceFile> + '_ {
        // TODO: Return a Result, don't panic.
        // TODO: Maybe don't eagerly read them here...?
        let top_sources = cargo_metadata_sources(&self.root).unwrap();
        let source_paths =
            indirect_sources(&self.root, top_sources.as_slice(), &options.globset).unwrap();
        let root = self.root.clone();
        source_paths.into_iter().filter_map(move |p| {
            SourceFile::new(&root, &p)
                .map_err(|err| {
                    eprintln!("error reading source {}: {}", p.to_slash_lossy(), err);
                })
                .ok()
        })
    }

    /// Return the path (possibly relative) to the root of the source tree.
    pub fn path(&self) -> &Path {
        &self.root
    }
}

fn indirect_sources(
    root_dir: &Path,
    top_sources: &[PathBuf],
    globset: &Option<GlobSet>,
) -> Result<BTreeSet<PathBuf>> {
    let dirs: BTreeSet<&Path> = top_sources.iter().map(|p| p.parent().unwrap()).collect();
    let mut files: BTreeSet<PathBuf> = BTreeSet::new();
    for top_dir in dirs {
        for p in walkdir::WalkDir::new(root_dir.join(&top_dir))
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
            files.insert(p.to_owned());
        }
    }
    Ok(files)
}

/// Given a path to a cargo manifest, find all the directly-referenced source files.
fn cargo_metadata_sources(source_dir: &Path) -> Result<Vec<PathBuf>> {
    let manifest = source_dir.join("Cargo.toml");
    let mut found: Vec<PathBuf> = Vec::new();
    let abs_source = source_dir.canonicalize()?;
    let cmd = cargo_metadata::MetadataCommand::new()
        .manifest_path(&manifest)
        .exec()
        .context("run cargo metadata")?;
    // println!("root package:\n{:#?}", cmd.root_package());
    if let Some(pkg) = cmd.root_package() {
        for target in &pkg.targets {
            if target.kind == ["lib"] || target.kind == ["bin"] {
                // println!("  target {} relpath {}", target.name, target.src_path);
                // dbg!(&abs_source);
                if let Ok(relpath) = target.src_path.strip_prefix(&abs_source) {
                    // println!("  target {} relpath {relpath}", target.name);
                    let relpath: PathBuf = relpath.into();
                    if !found.contains(&relpath) {
                        found.push(relpath);
                    }
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
        let source_paths = SourceTree::new(Path::new("testdata/tree/factorial"))
            .unwrap()
            .source_files(&Options::default())
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
        let source_file = SourceFile::new(temp.path(), Path::new(file_name)).unwrap();
        assert_eq!(*source_file.code, "fn main() {\n    640 << 10;\n}\n");
    }
}

// Copyright 2021 Martin Pool

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use path_slash::PathExt;

/// The path of a file within a source tree.
///
/// It can be viewed either relative to the source tree (for display)
/// or as a path that can be opened (relative to cwd or absolute.)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourcePath {
    tree_relative: PathBuf,
    pub full_path: PathBuf,
}

impl SourcePath {
    pub fn tree_relative_slashes(&self) -> String {
        self.tree_relative.to_slash_lossy()
    }

    pub fn read_to_string(&self) -> Result<String> {
        std::fs::read_to_string(&self.full_path)
            .with_context(|| format!("failed to read source of {:?}", self.full_path))
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

    #[allow(dead_code)]
    pub fn source_file(&self, tree_relative: &Path) -> SourcePath {
        SourcePath {
            tree_relative: tree_relative.to_owned(),
            full_path: self.root.join(tree_relative),
        }
    }

    /// Return an iterator of `src/**/*.rs` paths relative to the root.
    pub fn source_files(&self) -> impl Iterator<Item = SourcePath> + '_ {
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
            .map(move |full_path| {
                let tree_relative = full_path.strip_prefix(&self.root).unwrap().to_owned();
                SourcePath {
                    full_path,
                    tree_relative,
                }
            })
    }

    /// Return the path (possibly relative) to the root of the source tree.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn source_files_in_testdata_factorial() {
        let source_paths = SourceTree::new(Path::new("testdata/tree/factorial"))
            .unwrap()
            .source_files()
            .collect::<Vec<SourcePath>>();
        assert_eq!(source_paths.len(), 1);
        assert_eq!(
            source_paths[0],
            SourcePath {
                full_path: PathBuf::from("testdata/tree/factorial/src/bin/main.rs"),
                tree_relative: PathBuf::from("src/bin/main.rs"),
            }
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
}

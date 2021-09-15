// Copyright 2021 Martin Pool

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use path_slash::PathExt;

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

    /// Return an iterator of `src/**/*.rs` paths relative to the root.
    pub fn source_files(&self) -> impl Iterator<Item = PathBuf> + '_ {
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
            .map(move |path| path.strip_prefix(&self.root).unwrap().to_owned())
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
        assert_eq!(
            SourceTree::new(Path::new("testdata/tree/factorial"))
                .unwrap()
                .source_files()
                .collect::<Vec<PathBuf>>(),
            vec![Path::new("src/bin/main.rs")]
        )
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

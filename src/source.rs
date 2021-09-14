// Copyright 2021 Martin Pool

use std::path::{Path, PathBuf};

pub struct SourceTree {
    root: PathBuf,
}

impl SourceTree {
    pub fn new(root: &Path) -> SourceTree {
        SourceTree {
            root: root.to_owned(),
        }
    }
    /// Return an iterator through `src/**/*.rs`
    pub fn source_files(&self) -> impl Iterator<Item = PathBuf> {
        // TODO: Check there's a Cargo.toml.
        walkdir::WalkDir::new(self.root.join("src"))
            .sort_by_file_name()
            .into_iter()
            .filter_map(|r| r.ok())
            .filter(|entry| entry.file_type().is_file())
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .map_or(false, |p| p.eq_ignore_ascii_case("rs"))
            })
            .map(|entry| entry.into_path())
    }
}

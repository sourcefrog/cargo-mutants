// Copyright 2021 Martin Pool

use std::path::{Path, PathBuf};

pub struct SourceTree {
    root: PathBuf,
}

impl SourceTree {
    pub fn new(root: &Path) -> SourceTree {
        // TODO: Check there's a Cargo.toml.
        SourceTree {
            root: root.to_owned(),
        }
    }
    /// Return an iterator of `src/**/*.rs` paths relative to the root.
    pub fn source_files(&self) -> impl Iterator<Item = PathBuf> + '_ {
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
            .map(move |path| path.strip_prefix(&self.root).unwrap().to_owned())
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
                .source_files()
                .collect::<Vec<PathBuf>>(),
            vec![Path::new("src/bin/main.rs")]
        )
    }
}

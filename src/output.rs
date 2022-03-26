// Copyright 2021, 2022 Martin Pool

//! A `mutants.out` directory holding logs and other output.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::*;

const OUTDIR_NAME: &str = "mutants.out";
const ROTATED_NAME: &str = "mutants.out.old";

/// A `mutants.out` directory holding logs and other output information.
#[derive(Debug)]
pub struct OutputDir {
    path: PathBuf,
    log_dir: PathBuf,
}

impl OutputDir {
    /// Create a new `mutants.out` output directory, within the given directory.
    ///
    /// If the directory already exists, it's rotated to `mutants.out.old`. If that directory
    /// exists, it's deleted.
    pub fn new<P: AsRef<Path>>(in_dir: P) -> Result<OutputDir> {
        let path: PathBuf = in_dir.as_ref().join(OUTDIR_NAME);
        if path.exists() {
            let rotated = in_dir.as_ref().join(ROTATED_NAME);
            if rotated.exists() {
                fs::remove_dir_all(&rotated).with_context(|| format!("remove {:?}", &rotated))?;
            }
            fs::rename(&path, &rotated)
                .with_context(|| format!("move {:?} to {:?}", &path, &rotated))?;
        }
        fs::create_dir(&path).with_context(|| format!("create output directory {:?}", &path))?;
        let log_dir = path.join("log");
        fs::create_dir(&log_dir).with_context(|| format!("create log directory {:?}", &log_dir))?;
        Ok(OutputDir { path, log_dir })
    }

    /// Create a new log for a given scenario.
    ///
    /// Returns the [File] to which subprocess output should be sent, and a LogFile to read it
    /// later.
    pub fn create_log(&self, scenario: &Scenario) -> Result<LogFile> {
        LogFile::create_in(&self.log_dir, &scenario.log_file_name_base())
    }

    #[allow(dead_code)]
    /// Return the path of the `mutants.out` directory.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod test {
    use itertools::Itertools;
    use path_slash::PathExt;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use super::*;
    use crate::source::SourceTree;

    fn minimal_source_tree() -> TempDir {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), b"# enough for a test").unwrap();
        tmp
    }

    fn list_recursive(path: &Path) -> Vec<String> {
        walkdir::WalkDir::new(path)
            .sort_by_file_name()
            .into_iter()
            .map(|entry| {
                entry
                    .unwrap()
                    .path()
                    .strip_prefix(path)
                    .unwrap()
                    .to_slash_lossy()
            })
            .collect_vec()
    }

    #[test]
    fn create() {
        let tmp = minimal_source_tree();
        let src_tree = SourceTree::new(tmp.path()).unwrap();
        let output_dir = OutputDir::new(src_tree.root()).unwrap();
        assert_eq!(
            list_recursive(tmp.path()),
            &["", "Cargo.toml", "mutants.out", "mutants.out/log",]
        );
        assert_eq!(output_dir.path(), tmp.path().join("mutants.out"));
        assert_eq!(output_dir.log_dir, tmp.path().join("mutants.out/log"));
    }

    #[test]
    fn rotate() {
        let temp_dir = tempfile::TempDir::new().unwrap();

        // Create an initial output dir with one log.
        let output_dir = OutputDir::new(&temp_dir).unwrap();
        output_dir.create_log(&Scenario::SourceTree).unwrap();
        assert!(temp_dir
            .path()
            .join("mutants.out/log/source_tree.log")
            .is_file());

        // The second time we create it in the same directory, the old one is moved away.
        let output_dir = OutputDir::new(&temp_dir).unwrap();
        output_dir.create_log(&Scenario::SourceTree).unwrap();
        output_dir.create_log(&Scenario::Baseline).unwrap();
        assert!(temp_dir
            .path()
            .join("mutants.out.old/log/source_tree.log")
            .is_file());
        assert!(temp_dir
            .path()
            .join("mutants.out/log/source_tree.log")
            .is_file());
        assert!(temp_dir
            .path()
            .join("mutants.out/log/baseline.log")
            .is_file());

        // The third time (and later), the .old directory is removed.
        let output_dir = OutputDir::new(&temp_dir).unwrap();
        output_dir.create_log(&Scenario::SourceTree).unwrap();
        assert!(temp_dir
            .path()
            .join("mutants.out/log/source_tree.log")
            .is_file());
        assert!(!temp_dir
            .path()
            .join("mutants.out/log/baseline.log")
            .is_file());
        assert!(temp_dir
            .path()
            .join("mutants.out.old/log/source_tree.log")
            .is_file());
        assert!(temp_dir
            .path()
            .join("mutants.out.old/log/baseline.log")
            .is_file());
    }
}

// Copyright 2021 Martin Pool

//! A `mutants.out` directory holding logs and other output.
//!
//! *CAUTION:* This currently doesn't interact with Cargo locking, and if two `cargo-mutants`
//! processes access the same directory they'll tread on each other...

use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::{fs, io};

use anyhow::{Context, Result};

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
    pub fn create_log(&self, scenario_name: &str) -> Result<(File, LogFile)> {
        // TODO: Maybe remember what files have already been created to avoid this loop, although
        // realistically it seems unlikely to be hit often...
        let basename = clean_filename(scenario_name);
        for i in 0..1000 {
            let t = if i == 0 {
                format!("{}.log", basename)
            } else {
                format!("{}_{:03}.log", basename, i)
            };
            let path = self.log_dir.join(t);
            match fs::OpenOptions::new()
                .write(true)
                .read(true)
                .create_new(true)
                .open(&path)
            {
                Ok(file) => return Ok((file, LogFile { path })),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(anyhow::Error::from(e).context("create test log file")),
            }
        }
        unreachable!(
            "couldn't create any test log in {:?} for {:?}",
            self, scenario_name,
        );
    }

    #[allow(dead_code)]
    /// Return the path of the `mutants.out` directory.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn clean_filename(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' | '\\' | ' ' | ':' | '<' | '>' | '?' | '*' | '|' | '"' => '_',
            c => c,
        })
        .collect::<String>()
}

/// A log file for execution of a single scenario.
#[derive(Debug, Clone)]
pub struct LogFile {
    pub path: PathBuf,
}

impl LogFile {
    /// Return the full content of the log as a string.
    pub fn log_content(&self) -> Result<String> {
        let mut buf: Vec<u8> = Vec::new();
        File::open(&self.path)
            .and_then(|mut f| f.read_to_end(&mut buf))
            .with_context(|| format!("read log file {}", self.path.display()))?;
        Ok(String::from_utf8_lossy(&buf).into_owned())
    }

    /// Open the log file to append more content.
    pub fn open_append(&self) -> Result<File> {
        OpenOptions::new()
            .append(true)
            .open(&self.path)
            .with_context(|| format!("open {} for append", self.path.display()))
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
        output_dir.create_log("one").unwrap();
        assert!(temp_dir.path().join("mutants.out/log/one.log").is_file());

        // The second time we create it in the same directory, the old one is moved away.
        let output_dir = OutputDir::new(&temp_dir).unwrap();
        output_dir.create_log("two").unwrap();
        assert!(temp_dir
            .path()
            .join("mutants.out.old/log/one.log")
            .is_file());
        assert!(temp_dir.path().join("mutants.out/log/two.log").is_file());
        assert!(!temp_dir.path().join("mutants.out/log/one.log").is_file());

        // The third time (and later), the .old directory is removed.
        let output_dir = OutputDir::new(&temp_dir).unwrap();
        output_dir.create_log("three").unwrap();
        assert!(temp_dir.path().join("mutants.out/log/three.log").is_file());
        assert!(!temp_dir.path().join("mutants.out/log/two.log").is_file());
        assert!(!temp_dir.path().join("mutants.out/log/one.log").is_file());
        assert!(temp_dir
            .path()
            .join("mutants.out.old/log/two.log")
            .is_file());
        assert!(!temp_dir
            .path()
            .join("mutants.out.old/log/one.log")
            .is_file());
    }
}

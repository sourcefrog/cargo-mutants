// Copyright 2021 Martin Pool

//! A `target/enucleate` directory holding logs and other output.
//!
//! This currently doesn't interact with Cargo locking, and if two `enucleate`
//! runs access the same directory they'll tread on each other...

use std::fs::File;
use std::io::{Read, Seek};
use std::path::PathBuf;
use std::{fs, io};

use anyhow::{Context, Result};

use crate::source::SourceTree;

#[derive(Debug)]
pub struct OutputDir {
    pub path: PathBuf,
    pub log_dir: PathBuf,
}

impl OutputDir {
    pub fn new(tree: &SourceTree) -> Result<OutputDir> {
        let path: PathBuf = tree.root().join("target").join("enucleate");
        fs::create_dir_all(&path)
            .with_context(|| format!("create output directory {:?}", &path))?;
        let log_dir = path.join("log");
        fs::create_dir_all(&log_dir)
            .with_context(|| format!("create log directory {:?}", &log_dir))?;
        Ok(OutputDir { path, log_dir })
    }

    pub fn delete_logs(&self) -> Result<()> {
        for entry in self.log_dir.read_dir()? {
            let path = entry?.path();
            fs::remove_file(&path).with_context(|| format!("delete log file {:?}", &path))?;
        }
        Ok(())
    }

    pub fn create_log(&self, scenario_name: &str) -> Result<TestLog> {
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
                Ok(file) => return Ok(TestLog { path, file }),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(anyhow::Error::from(e).context("create test log file")),
            }
        }
        unreachable!(
            "couldn't create any test log in {:?} for {:?}",
            self, scenario_name,
        );
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

/// A log file for execution of a single test
#[derive(Debug)]
pub struct TestLog {
    pub path: PathBuf,
    pub file: File,
}

impl TestLog {
    pub fn log_content(&mut self) -> Result<String> {
        self.file.rewind()?;
        let mut bytes = Vec::new();
        self.file.read_to_end(&mut bytes)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}

#[cfg(test)]
mod test {
    use itertools::Itertools;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use super::*;

    fn minimal_source_tree() -> TempDir {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), b"# enough for a test").unwrap();
        tmp
    }

    #[test]
    fn create() {
        let tmp = minimal_source_tree();
        let src_tree = SourceTree::new(tmp.path()).unwrap();
        let _output_dir = OutputDir::new(&src_tree).unwrap();
        let names = walkdir::WalkDir::new(tmp.path())
            .sort_by_file_name()
            .into_iter()
            .map(|entry| {
                entry
                    .unwrap()
                    .path()
                    .strip_prefix(tmp.path())
                    .unwrap()
                    .display()
                    .to_string()
            })
            .collect_vec();

        assert_eq!(
            names,
            &[
                "",
                "Cargo.toml",
                "target",
                "target/enucleate",
                "target/enucleate/log"
            ]
        );
    }

    #[test]
    fn delete_existing_logs() {
        let tmp = minimal_source_tree();
        let src_tree = SourceTree::new(tmp.path()).unwrap();
        let output_dir = OutputDir::new(&src_tree).unwrap();
        let log_file_path = output_dir.log_dir.join("something.log");
        fs::write(&log_file_path, b"stuff\n").unwrap();
        assert!(log_file_path.is_file());
        output_dir.delete_logs().unwrap();
        assert!(!log_file_path.is_file());
    }
}

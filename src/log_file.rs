// Copyright 2021, 2022 Martin Pool

//! Manage per-scenario log files, which contain the output from cargo
//! and test cases, mixed with commentary from cargo-mutants.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::Result;

/// Text inserted in log files to make important sections more visible.
pub const LOG_MARKER: &str = "***";

/// A log file for execution of a single scenario.
#[derive(Debug)]
pub struct LogFile {
    path: PathBuf,
    write_to: File,
}

impl LogFile {
    pub fn create_in(log_dir: &Path, scenario_name: &str) -> Result<LogFile> {
        // TODO: Maybe remember what files have already been created to avoid this loop, although
        // realistically it seems unlikely to be hit often...
        let basename = clean_filename(scenario_name);
        for i in 0..1000 {
            let t = if i == 0 {
                format!("{}.log", basename)
            } else {
                format!("{}_{:03}.log", basename, i)
            };
            let path = log_dir.join(t);
            match OpenOptions::new()
                .write(true)
                .read(true)
                .create_new(true)
                .open(&path)
            {
                Ok(write_to) => return Ok(LogFile { path, write_to }),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(anyhow::Error::from(e).context("create test log file")),
            }
        }
        unreachable!(
            "couldn't create any test log in {:?} for {:?}",
            log_dir, scenario_name,
        );
    }

    /// Return the full content of the log as a string.
    #[allow(unused)]
    pub fn get_log_content(&self) -> Result<String> {
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

    /// Write a message, with a marker. Ignore errors.
    pub fn message(&mut self, message: &str) {
        write!(self.write_to, "\n{} {}", LOG_MARKER, message).expect("write message to log");
    }

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

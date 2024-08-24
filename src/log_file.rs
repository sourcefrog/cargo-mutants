// Copyright 2021-2023 Martin Pool

//! Manage per-scenario log files, which contain the output from cargo
//! and test cases, mixed with commentary from cargo-mutants.

use std::fs::{File, OpenOptions};
use std::io::Write;

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};

use crate::Result;

/// Text inserted in log files to make important sections more visible.
pub const LOG_MARKER: &str = "***";

/// A log file for execution of a single scenario.
#[derive(Debug)]
pub struct LogFile {
    path: Utf8PathBuf,
    write_to: File,
}

impl LogFile {
    pub fn create_in(log_dir: &Utf8Path, basename: &str) -> Result<LogFile> {
        let path = log_dir.join(format!("{basename}.log"));
        let write_to = OpenOptions::new()
            .create_new(true)
            .read(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("create test log file {path:?}"))?;
        Ok(LogFile { path, write_to })
    }

    /// Open the log file to append more content.
    pub fn open_append(&self) -> Result<File> {
        OpenOptions::new()
            .append(true)
            .open(&self.path)
            .with_context(|| format!("open {} for append", self.path))
    }

    /// Write a message, with a marker. Ignore errors.
    pub fn message(&mut self, message: &str) {
        write!(self.write_to, "\n{LOG_MARKER} {message}\n").expect("write message to log");
    }

    pub fn path(&self) -> &Utf8Path {
        &self.path
    }
}

pub fn clean_filename(s: &str) -> String {
    s.replace('/', "__")
        .chars()
        .map(|c| match c {
            '\\' | ' ' | ':' | '<' | '>' | '?' | '*' | '|' | '"' => '_',
            c => c,
        })
        .collect::<String>()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn clean_filename_removes_special_characters() {
        assert_eq!(
            clean_filename("1/2\\3:4<5>6?7*8|9\"0"),
            "1__2_3_4_5_6_7_8_9_0"
        );
    }
}

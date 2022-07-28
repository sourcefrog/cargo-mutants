// Copyright 2021, 2022 Martin Pool

//! Manage per-scenario log files, which contain the output from cargo
//! and test cases, mixed with commentary from cargo-mutants.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};

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
    pub fn create_in(log_dir: &Utf8Path, scenario_name: &str) -> Result<LogFile> {
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
                .append(true)
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

    /// Open the log file to append more content.
    pub fn open_append(&self) -> Result<File> {
        OpenOptions::new()
            .append(true)
            .open(&self.path)
            .with_context(|| format!("open {} for append", self.path))
    }

    /// Write a message, with a marker. Ignore errors.
    pub fn message(&mut self, message: &str) {
        write!(self.write_to, "\n{} {}\n", LOG_MARKER, message).expect("write message to log");
    }

    pub fn path(&self) -> &Utf8Path {
        &self.path
    }
}

/// Return the last non-empty line from a file, if it has any content.
pub fn last_line(path: &Utf8Path) -> Result<String> {
    // This is somewhat inefficient: we could potentially remember how long
    // the file was last time, seek to that point, and deal with incomplete
    // lines. However, probably these files will never get so colossal that
    // reading them is a big problem; they are almost certainly in cache;
    // and this should only be called a few times per second...
    Ok(fs::read_to_string(path)?
        .lines()
        .filter(|s| !s.trim().is_empty())
        .last()
        .unwrap_or_default()
        .to_owned())
}

fn clean_filename(s: &str) -> String {
    let s = s.replace('/', "__");
    s.chars()
        .map(|c| match c {
            '\\' | ' ' | ':' | '<' | '>' | '?' | '*' | '|' | '"' => '_',
            c => c,
        })
        .collect::<String>()
}

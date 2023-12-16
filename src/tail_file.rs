// Copyright 2021-2023 Martin Pool

//! Tail a log file: watch for new writes and return the last line.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use anyhow::Context;

use crate::Result;

#[derive(Debug)]
pub struct TailFile {
    file: File,
    last_line_seen: String,
    read_buf: Vec<u8>,
}

impl TailFile {
    /// Watch the given path.
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path.as_ref()).context("Open log file")?;
        Ok(TailFile {
            file,
            last_line_seen: String::new(),
            read_buf: Vec::new(),
        })
    }

    /// Return the last non-empty, non-whitespace line from this file, or an empty string
    /// if none have been seen yet.
    ///
    /// Non-UTF8 content is lost.
    pub fn last_line(&mut self) -> Result<&str> {
        // This assumes that the file always sees writes of whole lines, which seems
        // pretty likely: we don't attempt to stitch up writes of partial lines with
        // later writes, although we could...
        self.read_buf.clear();
        let n_read = self
            .file
            .read_to_end(&mut self.read_buf)
            .context("Read from log file")?;
        if n_read > 0 {
            if let Some(new_last) = String::from_utf8_lossy(&self.read_buf)
                .lines()
                .filter(|l| !l.trim().is_empty())
                .last()
            {
                self.last_line_seen = new_last.to_owned();
            }
        }
        Ok(self.last_line_seen.as_str())
    }
}

#[cfg(test)]
mod test {
    use camino::Utf8PathBuf;

    use std::io::Write;

    use super::*;

    #[test]
    fn last_line_of_file() {
        let mut tempfile = tempfile::NamedTempFile::new().unwrap();
        let path: Utf8PathBuf = tempfile.path().to_owned().try_into().unwrap();
        let mut tailer = TailFile::new(path).unwrap();

        assert_eq!(
            tailer.last_line().unwrap(),
            "",
            "empty file has an empty last line"
        );

        tempfile.write_all(b"hello").unwrap();
        assert_eq!(
            tailer.last_line().unwrap(),
            "hello",
            "single line file with no terminator has that line as last line"
        );

        tempfile.write_all(b"\n\n\n").unwrap();
        assert_eq!(
            tailer.last_line().unwrap(),
            "hello",
            "trailing blank lines are ignored"
        );

        tempfile.write_all(b"that's all folks!\n").unwrap();
        assert_eq!(
            tailer.last_line().unwrap(),
            "that's all folks!",
            "newline terminated last line is returned"
        );
    }
}

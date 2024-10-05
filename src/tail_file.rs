// Copyright 2021-2024 Martin Pool

//! Tail a log file: watch for new writes and return the last line.

use std::fs::File;
use std::io::Read;

use anyhow::Context;

use crate::Result;

/// Tail a log file, and return the last non-empty line seen.
///
/// This assumes that the log file always receives whole lines as atomic writes, which
/// is typical.  If the file is being written by a process that writes partial lines,
/// this won't panic or error but it may not return whole correct lines.
pub struct TailFile {
    file: File,
    /// The last non-empty line we've seen in the file so far.
    last_line_seen: String,
    read_buf: Vec<u8>,
}

impl TailFile {
    /// Watch lines appended to the given file, which should be open for reading.
    pub fn new(file: File) -> Result<Self> {
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
        self.read_buf.clear();
        let n_read = self
            .file
            .read_to_end(&mut self.read_buf)
            .context("Read tail of log file")?;
        if n_read > 0 {
            if let Some(new_last) = String::from_utf8_lossy(&self.read_buf)
                .lines()
                .filter(|l| !l.trim().is_empty())
                .last()
            {
                new_last.clone_into(&mut self.last_line_seen);
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
        let reopened = File::open(&path).unwrap();
        let mut tailer = TailFile::new(reopened).unwrap();

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

        tempfile.write_all(b"").unwrap();
        assert_eq!(
            tailer.last_line().unwrap(),
            "that's all folks!",
            "touched but unchanged file returns the same last line"
        );

        // These cases of partial writes aren't supported, because they don't seem to occur in
        // cargo/rustc output.

        // tempfile.write_all(b"word ").unwrap();
        // assert_eq!(
        //     tailer.last_line().unwrap(),
        //     "word ",
        //     "see one word from an incomplete line"
        // );

        // tempfile.write_all(b"word2 ").unwrap();
        // assert_eq!(
        //     tailer.last_line().unwrap(),
        //     "word word2 ",
        //     "see two words from an incomplete line"
        // );

        // tempfile.write_all(b"word3\n").unwrap();
        // assert_eq!(
        //     tailer.last_line().unwrap(),
        //     "word word2 word3",
        //     "the same line is continued and finished"
        // );
    }
}

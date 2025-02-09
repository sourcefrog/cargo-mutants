// Copyright 2021-2025 Martin Pool

//! A `mutants.out/lock.json` file indicating that the directory is in use.

use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

use fs2::FileExt;
use path_slash::PathExt;
use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tracing::info;

use crate::{check_interrupted, Context, Result};

use super::LOCK_FILENAME;

const LOCK_POLL: Duration = Duration::from_millis(100);

/// The contents of a `lock.json` written into the output directory and used as
/// a lock file to ensure that two cargo-mutants invocations don't try to write
/// to the same `mutants.out` simultneously.
#[derive(Serialize)]
pub struct LockFile {
    cargo_mutants_version: String,
    start_time: String,
    hostname: String,
    username: String,
}

impl LockFile {
    pub(super) fn new() -> LockFile {
        let start_time = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .expect("format current time");
        LockFile {
            cargo_mutants_version: crate::VERSION.to_string(),
            start_time,
            hostname: whoami::fallible::hostname().unwrap_or_default(),
            username: whoami::username(),
        }
    }

    /// Block until acquiring a file lock on `lock.json` in the given `mutants.out`
    /// directory.
    ///
    /// Return the `File` whose lifetime controls the file lock.
    pub fn acquire_lock(output_dir: &Path) -> Result<File> {
        let lock_path = output_dir.join(LOCK_FILENAME);
        let mut lock_file = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock_path)
            .context("open or create lock.json in existing directory")?;
        let mut first = true;
        while let Err(err) = lock_file.try_lock_exclusive() {
            if first {
                info!(
                    "Waiting for lock on {} ...: {err}",
                    lock_path.to_slash_lossy()
                );
                first = false;
            }
            check_interrupted()?;
            sleep(LOCK_POLL);
        }
        lock_file.set_len(0)?;
        lock_file
            .write_all(serde_json::to_string_pretty(&LockFile::new())?.as_bytes())
            .context("write lock.json")?;
        Ok(lock_file)
    }
}

// Copyright 2021-2023 Martin Pool

//! A `mutants.out` directory holding logs and other output.

use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

use anyhow::{Context, Result};
use camino::Utf8Path;
use fs2::FileExt;
use path_slash::PathExt;
use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tracing::info;

use crate::outcome::{LabOutcome, SummaryOutcome};
use crate::*;

const OUTDIR_NAME: &str = "mutants.out";
const ROTATED_NAME: &str = "mutants.out.old";
const LOCK_JSON: &str = "lock.json";
const LOCK_POLL: Duration = Duration::from_millis(100);

/// The contents of a `lock.json` written into the output directory and used as
/// a lock file to ensure that two cargo-mutants invocations don't try to write
/// to the same `mutants.out` simultneously.
#[derive(Debug, Serialize)]
struct LockFile {
    cargo_mutants_version: String,
    start_time: String,
    hostname: String,
    username: String,
}

impl LockFile {
    fn new() -> LockFile {
        let start_time = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .expect("format current time");
        LockFile {
            cargo_mutants_version: crate::VERSION.to_string(),
            start_time,
            hostname: whoami::hostname(),
            username: whoami::username(),
        }
    }

    /// Block until acquiring a file lock on `lock.json` in the given `mutants.out`
    /// directory.
    ///
    /// Return the `File` whose lifetime controls the file lock.
    pub fn acquire_lock(output_dir: &Path) -> Result<File> {
        let lock_path = output_dir.join(LOCK_JSON);
        let mut lock_file = File::options()
            .create(true)
            .write(true)
            .open(&lock_path)
            .context("open or create lock.json in existing directory")?;
        if lock_file.try_lock_exclusive().is_err() {
            info!("Waiting for lock on {} ...", lock_path.to_slash_lossy());
            let contended_kind = fs2::lock_contended_error().kind();
            loop {
                check_interrupted()?;
                if let Err(err) = lock_file.try_lock_exclusive() {
                    if err.kind() == contended_kind {
                        sleep(LOCK_POLL)
                    } else {
                        return Err(err).context("wait for lock");
                    }
                } else {
                    break;
                }
            }
        }
        lock_file.set_len(0)?;
        lock_file
            .write_all(serde_json::to_string_pretty(&LockFile::new())?.as_bytes())
            .context("write lock.json")?;
        Ok(lock_file)
    }
}

/// A `mutants.out` directory holding logs and other output information.
#[derive(Debug)]
pub struct OutputDir {
    path: Utf8PathBuf,
    log_dir: Utf8PathBuf,
    #[allow(unused)] // Lifetime controls the file lock
    lock_file: File,
    /// A file holding a list of missed mutants as text, one per line.
    missed_list: File,
    /// A file holding a list of caught mutants as text, one per line.
    caught_list: File,
    /// A file holding a list of mutants where testing timed out, as text, one per line.
    timeout_list: File,
    unviable_list: File,
    /// The accumulated overall lab outcome.
    pub lab_outcome: LabOutcome,
}

impl OutputDir {
    /// Create a new `mutants.out` output directory, within the given directory.
    ///
    /// If `in_dir` does not exist, it's created too, so that users can name a new directory
    /// with `--output`.
    ///
    /// If the directory already exists, it's rotated to `mutants.out.old`. If that directory
    /// exists, it's deleted.
    ///
    /// If the directory already exists and `lock.json` exists and is locked, this waits for
    /// the lock to be released. The returned `OutputDir` holds a lock for its lifetime.
    pub fn new(in_dir: &Utf8Path) -> Result<OutputDir> {
        if !in_dir.exists() {
            fs::create_dir(in_dir).context("create output parent directory {in_dir:?}")?;
        }
        let output_dir = in_dir.join(OUTDIR_NAME);
        if output_dir.exists() {
            LockFile::acquire_lock(output_dir.as_ref())?;
            // Now release the lock for a bit while we move the directory. This might be
            // slightly racy.

            let rotated = in_dir.join(ROTATED_NAME);
            if rotated.exists() {
                fs::remove_dir_all(&rotated).with_context(|| format!("remove {:?}", &rotated))?;
            }
            fs::rename(&output_dir, &rotated)
                .with_context(|| format!("move {:?} to {:?}", &output_dir, &rotated))?;
        }
        fs::create_dir(&output_dir)
            .with_context(|| format!("create output directory {:?}", &output_dir))?;
        let lock_file = LockFile::acquire_lock(output_dir.as_std_path())
            .context("create lock.json lock file")?;
        let log_dir = output_dir.join("log");
        fs::create_dir(&log_dir).with_context(|| format!("create log directory {:?}", &log_dir))?;

        // Create text list files.
        let mut list_file_options = OpenOptions::new();
        list_file_options.create(true).append(true);
        let missed_list = list_file_options
            .open(output_dir.join("missed.txt"))
            .context("create missed.txt")?;
        let caught_list = list_file_options
            .open(output_dir.join("caught.txt"))
            .context("create caught.txt")?;
        let unviable_list = list_file_options
            .open(output_dir.join("unviable.txt"))
            .context("create unviable.txt")?;
        let timeout_list = list_file_options
            .open(output_dir.join("timeout.txt"))
            .context("create timeout.txt")?;
        Ok(OutputDir {
            path: output_dir,
            lab_outcome: LabOutcome::new(),
            log_dir,
            lock_file,
            missed_list,
            caught_list,
            timeout_list,
            unviable_list,
        })
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
    pub fn path(&self) -> &Utf8Path {
        &self.path
    }

    /// Update the state of the overall lab.
    ///
    /// Called multiple times as the lab runs.
    fn write_lab_outcome(&self) -> Result<()> {
        serde_json::to_writer_pretty(
            BufWriter::new(File::create(self.path.join("outcomes.json"))?),
            &self.lab_outcome,
        )
        .context("write outcomes.json")
    }

    /// Add the result of testing one scenario.
    pub fn add_scenario_outcome(&mut self, scenario_outcome: &ScenarioOutcome) -> Result<()> {
        self.lab_outcome.add(scenario_outcome.to_owned());
        self.write_lab_outcome()?;
        let scenario = &scenario_outcome.scenario;
        if let Scenario::Mutant(mutant) = scenario {
            let file = match scenario_outcome.summary() {
                SummaryOutcome::MissedMutant => &mut self.missed_list,
                SummaryOutcome::CaughtMutant => &mut self.caught_list,
                SummaryOutcome::Timeout => &mut self.timeout_list,
                SummaryOutcome::Unviable => &mut self.unviable_list,
                _ => return Ok(()),
            };
            writeln!(file, "{}", mutant.name(true, false)).context("write to list file")?;
        }
        Ok(())
    }

    pub fn open_debug_log(&self) -> Result<File> {
        let debug_log_path = self.path.join("debug.log");
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&debug_log_path)
            .with_context(|| format!("open {debug_log_path}"))
    }

    pub fn write_mutants_list(&self, mutants: &[Mutant]) -> Result<()> {
        serde_json::to_writer_pretty(
            BufWriter::new(File::create(self.path.join("mutants.json"))?),
            mutants,
        )
        .context("write mutants.json")
    }

    pub fn take_lab_outcome(self) -> LabOutcome {
        self.lab_outcome
    }
}

#[cfg(test)]
mod test {
    use std::convert::TryInto;

    use indoc::indoc;
    use itertools::Itertools;
    use path_slash::PathExt;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use super::*;

    fn minimal_source_tree() -> TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path();
        fs::write(
            path.join("Cargo.toml"),
            indoc! { br#"
                # enough for a test
                [package]
                name = "cargo-mutants-minimal-test-tree"
                version = "0.0.0"
                "#
            },
        )
        .unwrap();
        fs::create_dir(path.join("src")).unwrap();
        fs::write(path.join("src/lib.rs"), b"fn foo() {}").unwrap();
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
                    .to_string()
            })
            .collect_vec()
    }

    #[test]
    fn create_output_dir() {
        let tmp = minimal_source_tree();
        let tmp_path: &Utf8Path = tmp.path().try_into().unwrap();
        let workspace = Workspace::open(tmp_path).unwrap();
        let output_dir = OutputDir::new(&workspace.dir).unwrap();
        assert_eq!(
            list_recursive(tmp.path()),
            &[
                "",
                "Cargo.toml",
                "mutants.out",
                "mutants.out/caught.txt",
                "mutants.out/lock.json",
                "mutants.out/log",
                "mutants.out/missed.txt",
                "mutants.out/timeout.txt",
                "mutants.out/unviable.txt",
                "src",
                "src/lib.rs",
            ]
        );
        assert_eq!(output_dir.path(), workspace.dir.join("mutants.out"));
        assert_eq!(output_dir.log_dir, workspace.dir.join("mutants.out/log"));
        assert!(output_dir.path().join("lock.json").is_file());
    }

    #[test]
    fn rotate() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let temp_dir_path = Utf8Path::from_path(temp_dir.path()).unwrap();

        // Create an initial output dir with one log.
        let output_dir = OutputDir::new(temp_dir_path).unwrap();
        output_dir.create_log(&Scenario::Baseline).unwrap();
        assert!(temp_dir
            .path()
            .join("mutants.out/log/baseline.log")
            .is_file());
        drop(output_dir); // release the lock.

        // The second time we create it in the same directory, the old one is moved away.
        let output_dir = OutputDir::new(temp_dir_path).unwrap();
        output_dir.create_log(&Scenario::Baseline).unwrap();
        assert!(temp_dir
            .path()
            .join("mutants.out.old/log/baseline.log")
            .is_file());
        assert!(temp_dir
            .path()
            .join("mutants.out/log/baseline.log")
            .is_file());
        drop(output_dir);

        // The third time (and later), the .old directory is removed.
        let output_dir = OutputDir::new(temp_dir_path).unwrap();
        output_dir.create_log(&Scenario::Baseline).unwrap();
        assert!(temp_dir
            .path()
            .join("mutants.out/log/baseline.log")
            .is_file());
        assert!(temp_dir
            .path()
            .join("mutants.out.old/log/baseline.log")
            .is_file());
        assert!(temp_dir
            .path()
            .join("mutants.out.old/log/baseline.log")
            .is_file());
    }
}

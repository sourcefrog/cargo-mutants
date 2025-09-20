// Copyright 2021-2024 Martin Pool

//! A `mutants.out` directory holding logs and other output.

use std::collections::{hash_map::Entry, HashMap};
use std::fs::{create_dir, read_to_string, remove_dir_all, rename, write, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};
use fs2::FileExt;
use jiff::Timestamp;
use path_slash::PathExt;
use serde::Serialize;
use tracing::{info, trace};

use crate::outcome::{LabOutcome, SummaryOutcome};
use crate::{check_interrupted, Context, Mutant, Result, Scenario, ScenarioOutcome};

const OUTDIR_NAME: &str = "mutants.out";
const ROTATED_NAME: &str = "mutants.out.old";
const LOCK_JSON: &str = "lock.json";
const LOCK_POLL: Duration = Duration::from_millis(100);
static CAUGHT_TXT: &str = "caught.txt";
static PREVIOUSLY_CAUGHT_TXT: &str = "previously_caught.txt";
static UNVIABLE_TXT: &str = "unviable.txt";

/// The contents of a `lock.json` written into the output directory and used as
/// a lock file to ensure that two cargo-mutants invocations don't try to write
/// to the same `mutants.out` simultneously.
#[derive(Debug, Serialize)]
struct LockFile {
    cargo_mutants_version: String,
    start_time: Timestamp,
    hostname: String,
    username: String,
}

impl LockFile {
    fn new() -> LockFile {
        LockFile {
            cargo_mutants_version: crate::VERSION.to_string(),
            start_time: Timestamp::now(),
            hostname: whoami::fallible::hostname().unwrap_or_default(),
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

/// A `mutants.out` directory holding logs and other output information.
#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct OutputDir {
    path: Utf8PathBuf,

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
    /// Log filenames which have already been used, and the number of times that each
    /// basename has been used.
    used_log_names: HashMap<String, usize>,
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
            create_dir(in_dir)
                .with_context(|| format!("create output parent directory {in_dir:?}"))?;
        }
        let output_dir = in_dir.join(OUTDIR_NAME);
        if output_dir.exists() {
            LockFile::acquire_lock(output_dir.as_ref())?;
            // Now release the lock for a bit while we move the directory. This might be
            // slightly racy.
            // TODO: Move the lock outside the directory, <https://github.com/sourcefrog/cargo-mutants/issues/402>.

            let rotated = in_dir.join(ROTATED_NAME);
            if rotated.exists() {
                remove_dir_all(&rotated).with_context(|| format!("remove {:?}", &rotated))?;
            }
            rename(&output_dir, &rotated)
                .with_context(|| format!("move {:?} to {:?}", &output_dir, &rotated))?;
        }
        create_dir(&output_dir)
            .with_context(|| format!("create output directory {:?}", &output_dir))?;
        let lock_file = LockFile::acquire_lock(output_dir.as_std_path())
            .context("create lock.json lock file")?;
        let log_dir = output_dir.join("log");
        create_dir(&log_dir).with_context(|| format!("create log directory {:?}", &log_dir))?;
        let diff_dir = output_dir.join("diff");
        create_dir(diff_dir).context("create diff dir")?;

        // Create text list files.
        let mut list_file_options = OpenOptions::new();
        list_file_options.create(true).append(true);
        let missed_list = list_file_options
            .open(output_dir.join("missed.txt"))
            .context("create missed.txt")?;
        let caught_list = list_file_options
            .open(output_dir.join(CAUGHT_TXT))
            .context("create caught.txt")?;
        let unviable_list = list_file_options
            .open(output_dir.join(UNVIABLE_TXT))
            .context("create unviable.txt")?;
        let timeout_list = list_file_options
            .open(output_dir.join("timeout.txt"))
            .context("create timeout.txt")?;
        Ok(OutputDir {
            path: output_dir,
            lab_outcome: LabOutcome::new(),
            lock_file,
            missed_list,
            caught_list,
            timeout_list,
            unviable_list,
            used_log_names: HashMap::new(),
        })
    }

    /// Allocate a sequence number and the output files for a scenario.
    pub fn start_scenario(&mut self, scenario: &Scenario) -> Result<ScenarioOutput> {
        let scenario_name = match scenario {
            Scenario::Baseline => "baseline".into(),
            Scenario::Mutant(mutant) => mutant.log_file_name_base(),
        };
        let basename = match self.used_log_names.entry(scenario_name.clone()) {
            Entry::Occupied(mut e) => {
                let index = e.get_mut();
                *index += 1;
                format!("{scenario_name}_{index:03}")
            }
            Entry::Vacant(e) => {
                e.insert(0);
                scenario_name
            }
        };
        ScenarioOutput::new(&self.path, scenario, &basename)
    }

    /// Return the path of the `mutants.out` directory.
    #[allow(unused)]
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
            writeln!(file, "{}", mutant.name(true)).context("write to list file")?;
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

    pub fn write_previously_caught(&self, caught: &[String]) -> Result<()> {
        let path = self.path.join(PREVIOUSLY_CAUGHT_TXT);
        let mut b = String::with_capacity(caught.iter().map(|l| l.len() + 1).sum());
        for l in caught {
            b.push_str(l);
            b.push('\n');
        }
        File::options()
            .create_new(true)
            .write(true)
            .open(&path)
            .and_then(|mut f| f.write_all(b.as_bytes()))
            .with_context(|| format!("Write {path:?}"))
    }
}

/// Return the string names of mutants previously caught in this output directory, including
/// unviable mutants.
///
/// Returns an empty vec if there are none.
pub fn load_previously_caught(output_parent_dir: &Utf8Path) -> Result<Vec<String>> {
    let mut r = Vec::new();
    for filename in [CAUGHT_TXT, UNVIABLE_TXT, PREVIOUSLY_CAUGHT_TXT] {
        let p = output_parent_dir.join(OUTDIR_NAME).join(filename);
        trace!(?p, "read previously caught");
        if p.is_file() {
            r.extend(
                read_to_string(&p)
                    .with_context(|| format!("Read previously caught mutants from {p:?}"))?
                    .lines()
                    .map(str::to_string),
            );
        }
    }
    Ok(r)
}

/// Where to write output about a particular Scenario.
#[allow(clippy::module_name_repetitions)]
pub struct ScenarioOutput {
    pub output_dir: Utf8PathBuf,
    log_path: Utf8PathBuf,
    pub log_file: File,
    /// File holding the diff of the mutated file, only if it's a mutation.
    pub diff_path: Option<Utf8PathBuf>,
}

impl ScenarioOutput {
    fn new(output_dir: &Utf8Path, scenario: &Scenario, basename: &str) -> Result<Self> {
        let log_path = Utf8PathBuf::from(format!("log/{basename}.log"));
        let log_file = File::options()
            .append(true)
            .create_new(true)
            .read(true)
            .open(output_dir.join(&log_path))?;
        let diff_path = if scenario.is_mutant() {
            Some(Utf8PathBuf::from(format!("diff/{basename}.diff")))
        } else {
            None
        };
        let mut scenario_output = Self {
            output_dir: output_dir.to_owned(),
            log_path,
            log_file,
            diff_path,
        };
        scenario_output.message(&scenario.to_string())?;
        Ok(scenario_output)
    }

    pub fn log_path(&self) -> &Utf8Path {
        &self.log_path
    }

    pub fn write_diff(&mut self, diff: &str) -> Result<()> {
        self.message(&format!("mutation diff:\n{diff}"))?;
        let diff_path = self.diff_path.as_ref().expect("should know the diff path");
        write(self.output_dir.join(diff_path), diff.as_bytes())
            .with_context(|| format!("write diff to {diff_path}"))
    }

    /// Open a new handle reading from the start of the log file.
    pub fn open_log_read(&self) -> Result<File> {
        let path = self.output_dir.join(&self.log_path);
        OpenOptions::new()
            .read(true)
            .open(&path)
            .with_context(|| format!("reopen {path} for read"))
    }

    /// Open a new handle that appends to the log file, so that it can be passed to a subprocess.
    pub fn open_log_append(&self) -> Result<File> {
        let path = self.output_dir.join(&self.log_path);
        OpenOptions::new()
            .append(true)
            .open(&path)
            .with_context(|| format!("reopen {path} for append"))
    }

    /// Write a message, with a marker.
    pub fn message(&mut self, message: &str) -> Result<()> {
        write!(self.log_file, "\n*** {message}\n").context("write message to log")
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
    use std::fs::write;

    use indoc::indoc;
    use itertools::Itertools;
    use pretty_assertions::assert_eq;
    use tempfile::{tempdir, TempDir};

    use super::*;
    use crate::workspace::Workspace;

    fn minimal_source_tree() -> TempDir {
        let tmp = tempdir().unwrap();
        let path = tmp.path();
        write(
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
        create_dir(path.join("src")).unwrap();
        write(path.join("src/lib.rs"), b"fn foo() {}").unwrap();
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
    fn clean_filename_removes_special_characters() {
        assert_eq!(
            clean_filename("1/2\\3:4<5>6?7*8|9\"0"),
            "1__2_3_4_5_6_7_8_9_0"
        );
    }

    #[test]
    fn create_output_dir() {
        let tmp = minimal_source_tree();
        let tmp_path: &Utf8Path = tmp.path().try_into().unwrap();
        let workspace = Workspace::open(tmp_path).unwrap();
        let output_dir = OutputDir::new(workspace.root()).unwrap();
        assert_eq!(
            list_recursive(tmp.path()),
            &[
                "",
                "Cargo.toml",
                "mutants.out",
                "mutants.out/caught.txt",
                "mutants.out/diff",
                "mutants.out/lock.json",
                "mutants.out/log",
                "mutants.out/missed.txt",
                "mutants.out/timeout.txt",
                "mutants.out/unviable.txt",
                "src",
                "src/lib.rs",
            ]
        );
        assert_eq!(output_dir.path(), workspace.root().join("mutants.out"));
        assert!(output_dir.path().join("lock.json").is_file());
    }

    #[test]
    fn rotate() {
        let temp_dir = TempDir::new().unwrap();
        let temp_dir_path = Utf8Path::from_path(temp_dir.path()).unwrap();

        // Create an initial output dir with one log.
        let mut output_dir = OutputDir::new(temp_dir_path).unwrap();
        let scenario_output = output_dir.start_scenario(&Scenario::Baseline).unwrap();
        assert!(temp_dir_path.join("mutants.out/log/baseline.log").is_file());
        drop(output_dir); // release the lock.
        drop(scenario_output);

        // The second time we create it in the same directory, the old one is moved away.
        let mut output_dir = OutputDir::new(temp_dir_path).unwrap();
        output_dir.start_scenario(&Scenario::Baseline).unwrap();
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
        let mut output_dir = OutputDir::new(temp_dir_path).unwrap();
        output_dir.start_scenario(&Scenario::Baseline).unwrap();
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

    #[test]
    fn track_previously_caught() {
        let temp_dir = TempDir::new().unwrap();
        let parent = Utf8Path::from_path(temp_dir.path()).unwrap();

        let example = "src/process.rs:213:9: replace ProcessStatus::is_success -> bool with true
src/process.rs:248:5: replace get_command_output -> Result<String> with Ok(String::new())
";

        // Read from an empty dir: succeeds.
        assert!(load_previously_caught(parent)
            .expect("load succeeds")
            .is_empty());

        let output_dir = OutputDir::new(parent).unwrap();
        assert!(load_previously_caught(parent)
            .expect("load succeeds")
            .is_empty());

        write(parent.join("mutants.out/caught.txt"), example.as_bytes()).unwrap();
        let previously_caught = load_previously_caught(parent).expect("load succeeds");
        assert_eq!(
            previously_caught.iter().collect_vec(),
            example.lines().collect_vec()
        );

        // make a new output dir, moving away the old one, and write this
        drop(output_dir);
        let output_dir = OutputDir::new(parent).unwrap();
        output_dir
            .write_previously_caught(&previously_caught)
            .unwrap();
        assert_eq!(
            read_to_string(parent.join("mutants.out/caught.txt")).expect("read caught.txt"),
            ""
        );
        assert!(parent.join("mutants.out/previously_caught.txt").is_file());
        let now = load_previously_caught(parent).expect("load succeeds");
        assert_eq!(now.iter().collect_vec(), example.lines().collect_vec());
    }
}

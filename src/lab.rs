// Copyright 2021 Martin Pool

//! A lab directory in which to test mutations to the source code, and control
//! over running `cargo`.

use std::path::PathBuf;
use std::process;
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use path_slash::PathExt;
use tempfile::TempDir;

use crate::console::{self, Activity};
use crate::mutate::Mutation;
use crate::outcome::{LabOutcome, Outcome, Status};
use crate::output::OutputDir;
use crate::source::SourceTree;

// Until we can reliably stop the grandchild test binaries, by killing a process
// group, timeouts are disabled.
const TEST_TIMEOUT: Duration = Duration::MAX; // Duration::from_secs(60);

/// Holds scratch directories in which files can be mutated and tests executed.
#[derive(Debug)]
pub struct Lab<'s> {
    source: &'s SourceTree,

    /// Top-level temporary directory for this lab.
    #[allow(unused)] // Needed to set tmpdir lifetime.
    tmp: TempDir,

    /// Path (within tmp) holding a copy of the source that can be modified and built.
    build_dir: PathBuf,

    /// Output directory, holding logs.
    output_dir: OutputDir,
}

impl<'s> Lab<'s> {
    pub fn new(source: &'s SourceTree) -> Result<Lab<'s>> {
        let tmp = TempDir::new()?;
        let build_dir = tmp.path().join("build");
        let activity = Activity::start("copy source to scratch directory");
        // I thought perhaps we could skip copying `/target` here, because Cargo
        // seems to rebuild some things even when given a full copy of the
        // target directory, with mtimes preserved. However, it does seem to
        // speed up the initial build enough to generally be worthwhile.
            .context("copy source tree to lab directory")
        {
            Ok(stats) => activity.succeed(&format!("{} MB", stats.file_bytes / 1_000_000)),
            Err(err) => {
                activity.fail("failed");
                eprintln!(
                    "error copying source tree {} to {}: {:?}",
                    &source.root().to_slash_lossy(),
                    &build_dir.to_slash_lossy(),
                    err
                );
                return Err(err);
            }
        }
        let output_dir = OutputDir::new(source)?;
        output_dir.delete_logs()?;
        Ok(Lab {
            source,
            tmp,
            build_dir,
            output_dir,
        })
    }

    /// Run all possible mutations in this lab.
    ///
    /// Before testing the mutations, the lab checks that the source tree passes its tests with
    /// no mutations applied.
    pub fn run(&self) -> Result<LabOutcome> {
        let mut lab_outcome = LabOutcome::default();
        let clean_outcome = self.test_clean()?;
        lab_outcome.add(&clean_outcome);
        if clean_outcome.status != Status::CleanTestPassed {
            console::print_error(
                "tests failed in a clean copy of the tree, so no mutants were tested",
            );
            return Ok(lab_outcome);
        }
        // TODO: Handle failure of clean build by returning a result?
        for mutation in self.source.mutations()? {
            lab_outcome.add(&self.test_mutation(&mutation)?);
        }
        Ok(lab_outcome)
    }

    /// Test building the unmodified source.
    ///
    /// If there are already-failing tests, proceeding to test mutations
    /// won't give a clear signal.
    pub fn test_clean(&self) -> Result<Outcome> {
        let activity = Activity::start("baseline test with no mutations");
        let outcome = self.run_cargo_test("baseline", &activity, Status::from_clean_test)?;
        activity.outcome(&outcome);
        if outcome.status != Status::CleanTestPassed {
            print!("{}", &outcome.log_content);
        }
        Ok(outcome)
    }

    /// Test with one mutation applied.
    pub fn test_mutation(&self, mutation: &Mutation) -> Result<Outcome> {
        let activity = Activity::start_mutation(mutation);
        // TODO: Maybe an object representing the applied mutation that reverts
        // on Drop?
        mutation.apply_in_dir(&self.build_dir)?;
        let test_result =
            self.run_cargo_test(&mutation.to_string(), &activity, Status::from_mutant_test);
        // Revert even if there was an error running cargo test
        mutation.revert_in_dir(&self.build_dir)?;
        let outcome = test_result?;
        activity.outcome(&outcome);
        Ok(outcome)
    }

    fn run_cargo_test(
        &self,
        scenario_name: &str,
        activity: &Activity,
        status_interpretation: fn(process::ExitStatus) -> Status,
    ) -> Result<Outcome> {
        let start = Instant::now();
        let mut timed_out = false;
        let mut log_file = self.output_dir.create_log(scenario_name)?;
        let mut child = Command::new("cargo")
            .arg("test")
            .current_dir(&self.build_dir)
            .stdout(log_file.file.try_clone()?)
            .stderr(log_file.file.try_clone()?)
            .stdin(process::Stdio::null())
            .spawn()
            .context("spawn cargo test")?;
        let exit_status = loop {
            if start.elapsed() > TEST_TIMEOUT {
                // eprintln!("bored! killing child...");
                if let Err(e) = child.kill() {
                    // most likely we raced and it's already gone
                    eprintln!("failed to kill child after timeout: {}", e);
                }
                timed_out = true;
                // Give it a bit of time to exit, then keep signalling until it
                // does stop.
                sleep(Duration::from_millis(200));
            }
            match child.try_wait()? {
                Some(status) => break status,
                None => sleep(Duration::from_millis(200)),
            }
            activity.tick();
        };
        Ok(Outcome {
            status: if timed_out {
                Status::Timeout
            } else {
                status_interpretation(exit_status)
            },
            log_content: log_file.log_content()?,
            duration: start.elapsed(),
        })
    }
}

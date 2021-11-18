// Copyright 2021 Martin Pool

//! A lab directory in which to test mutations to the source code, and control
//! over running `cargo`.

use std::borrow::Cow;
use std::collections::HashMap;
use std::env;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{self, Command, ExitStatus};
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use path_slash::PathExt;
use tempfile::TempDir;

use crate::console::{self, Activity, Console};
use crate::exit_code;
use crate::mutate::Mutation;
use crate::output::{LogFile, OutputDir};
use crate::source::SourceTree;

// Until we can reliably stop the grandchild test binaries, by killing a process
// group, timeouts are disabled.
const TEST_TIMEOUT: Duration = Duration::MAX; // Duration::from_secs(60);

/// Text inserted in log files to make important sections more visible.
const LOG_MARKER: &str = "***";

/// Run all possible mutation experiments.
///
/// Before testing the mutations, the lab checks that the source tree passes its tests with no
/// mutations applied.
pub fn experiment(source_tree: &SourceTree, console: &Console) -> Result<LabOutcome> {
    let tmp_dir = TempDir::new()?;
    let build_dir = copy_source_to_scratch(source_tree, tmp_dir.path(), console)?;
    let output_dir = OutputDir::new(source_tree.root())?;
    let mut lab_outcome = LabOutcome::default();

    let clean_outcome = test_clean(&build_dir, &output_dir, console)?;
    lab_outcome.add(&clean_outcome);
    if clean_outcome.status != Status::CleanTestPassed {
        console::print_error("tests failed in a clean copy of the tree, so no mutants were tested");
        return Ok(lab_outcome); // TODO: Maybe should be Err?
    }

    for mutation in source_tree.mutations()? {
        lab_outcome.add(&test_mutation(&mutation, &build_dir, &output_dir, console)?);
    }
    Ok(lab_outcome)
}

/// The bottom line of trying a mutation: it was caught, missed, failed to build, etc.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
#[must_use]
pub enum Status {
    // TODO: Maybe these would be better as an Error type and in the Err branch of a Result?
    /// The mutation was caught by tests.
    MutantCaught,
    /// The mutation was not caught by any tests.
    MutantMissed,
    /// Test ran too long and was killed. Maybe the mutation caused an infinite
    /// loop.
    Timeout,
    /// The tests are already failing in a clean tree.
    CleanTestFailed,
    /// Tests passed in a clean tree.
    CleanTestPassed,
    CheckFailed,
    BuildFailed,
}

impl Status {
    pub fn from_mutant_test(exit_status: process::ExitStatus) -> Status {
        // TODO: Detect signals and cargo failures other than test failures.
        if exit_status.success() {
            Status::MutantMissed
        } else {
            Status::MutantCaught
        }
    }

    pub fn from_clean_test(exit_status: process::ExitStatus) -> Status {
        if exit_status.success() {
            Status::CleanTestPassed
        } else {
            Status::CleanTestFailed
        }
    }
}

/// The outcome from a whole lab run containing multiple mutants.
#[derive(Debug, Default)]
pub struct LabOutcome {
    count_by_status: HashMap<Status, usize>,
}

impl LabOutcome {
    /// Record the event of one test.
    pub fn add(&mut self, outcome: &Outcome) {
        self.count_by_status
            .entry(outcome.status)
            .and_modify(|c| *c += 1)
            .or_insert(1);
    }

    /// Return the count of tests that failed with the given status.
    pub fn count(&self, status: Status) -> usize {
        self.count_by_status
            .get(&status)
            .cloned()
            .unwrap_or_default()
    }

    /// Return the overall program exit code reflecting this outcome.
    pub fn exit_code(&self) -> i32 {
        use Status::*;
        if self.count(CleanTestFailed) > 0 {
            exit_code::CLEAN_TESTS_FAILED
        } else if self.count(Timeout) > 0 {
            exit_code::TIMEOUT
        } else if self.count(MutantMissed) > 0 {
            exit_code::FOUND_PROBLEMS
        } else {
            exit_code::SUCCESS
        }
    }
}

/// Test building the unmodified source.
///
/// If there are already-failing tests, proceeding to test mutations
/// won't give a clear signal.
fn test_clean(build_dir: &Path, output_dir: &OutputDir, console: &Console) -> Result<Outcome> {
    let mut activity = console.start_activity("baseline test with no mutations");
    let scenario_name = "baseline";
    let (mut out_file, log_file) = output_dir.create_log(scenario_name)?;
    writeln!(out_file, "{} {}", LOG_MARKER, scenario_name)?;
    let outcome = run_scenario(build_dir, &mut activity, &log_file, true)?;
    activity.outcome(&outcome)?;
    Ok(outcome)
}

/// Test with one mutation applied.
fn test_mutation(
    mutation: &Mutation,
    build_dir: &Path,
    output_dir: &OutputDir,
    console: &Console,
) -> Result<Outcome> {
    let mut activity = console.start_mutation(mutation);
    let scenario_name = &mutation.to_string();
    let (mut out_file, log_file) = output_dir.create_log(scenario_name)?;
    writeln!(out_file, "{} {}", LOG_MARKER, scenario_name)?;
    writeln!(out_file, "{}", mutation.diff())?;
    let outcome = mutation.with_mutation_applied(build_dir, || {
        run_scenario(build_dir, &mut activity, &log_file, false)
    })?;
    activity.outcome(&outcome)?;
    Ok(outcome)
}

/// The result of running one mutation scenario.
#[derive(Debug)]
#[must_use]
pub struct Outcome {
    /// High-level categorization of what happened.
    pub status: Status,
    /// A file holding the text output from running this test.
    pub log_file: LogFile,
    pub duration: Duration,
}

impl Outcome {
    pub fn new(log_file: &LogFile, start_time: &Instant, status: Status) -> Outcome {
        Outcome {
            log_file: log_file.clone(),
            duration: start_time.elapsed(),
            status,
        }
    }
}

/// Successively run cargo check, build, test, and return the overall outcome.
fn run_scenario(
    build_dir: &Path,
    activity: &mut Activity,
    log_file: &LogFile,
    is_clean: bool,
) -> Result<Outcome> {
    // TODO: Maybe separate launching and collecting the result, so
    // that we can run several in parallel.

    let start = Instant::now();

    activity.set_phase("check");
    if !run_cargo("check", build_dir, activity, log_file)?.success() {
        return Ok(Outcome::new(log_file, &start, Status::CheckFailed));
    }

    // TODO: Actually `build --tests`, etc?
    activity.set_phase("build");
    if !run_cargo("build", build_dir, activity, log_file)?.success() {
        return Ok(Outcome::new(log_file, &start, Status::BuildFailed));
    }

    activity.set_phase("test");
    let test_result = run_cargo("test", build_dir, activity, log_file)?;
    let status = if is_clean {
        Status::from_clean_test(test_result.exit_status)
    } else {
        Status::from_mutant_test(test_result.exit_status)
    };

    Ok(Outcome::new(log_file, &start, status))
}

/// The result of running a single Cargo command.
struct CargoResult {
    timed_out: bool,
    exit_status: ExitStatus,
}

impl CargoResult {
    fn success(&self) -> bool {
        !self.timed_out && self.exit_status.success()
    }
}

fn run_cargo(
    cargo_subcommand: &str,
    in_dir: &Path,
    activity: &mut Activity,
    log_file: &LogFile,
) -> Result<CargoResult> {
    let start = Instant::now();
    let mut timed_out = false;
    // When run as a Cargo subcommand, which is the usual/intended case,
    // $CARGO tells us the right way to call back into it, so that we get
    // the matching toolchain etc.
    let cargo_bin: Cow<str> = env::var("CARGO")
        .map(Cow::from)
        .unwrap_or(Cow::Borrowed("cargo"));
    let mut out_file = log_file.open_append()?;
    writeln!(
        out_file,
        "\n{} run {} {}",
        LOG_MARKER, cargo_bin, cargo_subcommand
    )?;

    let mut child = Command::new(cargo_bin.as_ref())
        .arg(cargo_subcommand)
        .current_dir(in_dir)
        .stdout(out_file.try_clone()?)
        .stderr(out_file.try_clone()?)
        .stdin(process::Stdio::null())
        .spawn()
        .with_context(|| format!("failed to spawn {} {}", cargo_bin, cargo_subcommand))?;
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
    let duration = start.elapsed();
    writeln!(
        out_file,
        "\n{} cargo result: {:?} in {:?}",
        LOG_MARKER, exit_status, duration
    )?;
    Ok(CargoResult {
        timed_out,
        exit_status,
    })
}

fn copy_source_to_scratch(
    source: &SourceTree,
    tmp_path: &Path,
    console: &Console,
) -> Result<PathBuf> {
    let build_dir = tmp_path.join("build");
    let mut activity = console.start_copy_activity("copy source to scratch directory");
    // I thought we could skip copying /target here, but it turns out that copying
    // it does speed up the first build.
    match cp_r::CopyOptions::new()
        .after_entry_copied(|_path, _ft, stats| {
            activity.bytes_copied(stats.file_bytes);
        })
        .copy_tree(source.root(), &build_dir)
        .context("copy source tree to lab directory")
    {
        Ok(stats) => activity.succeed(stats.file_bytes),
        Err(err) => {
            activity.fail();
            eprintln!(
                "error copying source tree {} to {}: {:?}",
                &source.root().to_slash_lossy(),
                &build_dir.to_slash_lossy(),
                err
            );
            return Err(err);
        }
    }
    Ok(build_dir)
}

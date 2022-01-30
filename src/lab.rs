// Copyright 2021, 2022 Martin Pool

//! Successively apply mutations to the source code and run cargo to check, build, and test them.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use path_slash::PathExt;
use tempfile::TempDir;

use crate::console::{self, Activity, Console};
use crate::exit_code;
use crate::log_file::LogFile;
use crate::mutate::Mutation;
use crate::output::OutputDir;
use crate::run::{run_cargo, CargoResult};
use crate::source::SourceTree;

/// Options for running experiments.
#[derive(Default, Debug)]
pub struct ExperimentOptions {
    /// Don't run the tests, just see if each mutant builds.
    pub check_only: bool,
}

/// Run all possible mutation experiments.
///
/// Before testing the mutations, the lab checks that the source tree passes its tests with no
/// mutations applied.
pub fn experiment(
    source_tree: &SourceTree,
    options: &ExperimentOptions,
    console: &Console,
) -> Result<LabOutcome> {
    let mut lab_outcome = LabOutcome::default();
    let output_dir = OutputDir::new(source_tree.root())?;
    build_source_tree(source_tree, &output_dir, options, console)?;

    let tmp_dir = TempDir::new()?;
    let build_dir = copy_source_to_scratch(source_tree, tmp_dir.path(), console)?;

    let clean_outcome = test_clean(&build_dir, &output_dir, options, console)?;
    lab_outcome.add(&clean_outcome);
    if !clean_outcome.status.passed() {
        console::print_error("tests failed in a clean copy of the tree, so no mutants were tested");
        return Ok(lab_outcome); // TODO: Maybe should be Err?
    }

    let mutations = source_tree.mutations()?;
    serde_json::to_writer_pretty(
        BufWriter::new(File::create(output_dir.path().join("mutants.json"))?),
        &mutations,
    )?;
    for mutation in mutations {
        lab_outcome.add(&test_mutation(
            &mutation,
            &build_dir,
            &output_dir,
            options,
            console,
        )?);
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
    /// Only `cargo check` was run, and it passed.
    CheckPassed,
    BuildFailed,
    /// Build failed in the original source tree.
    SourceBuildFailed,
    /// Build passed in the original source tree.
    SourceBuildPassed,
}

impl Status {
    pub fn from_mutant_test(cargo_result: &CargoResult) -> Status {
        // TODO: Detect signals and cargo failures other than test failures.
        match cargo_result {
            CargoResult::Timeout => Status::Timeout,
            CargoResult::Success => Status::MutantMissed,
            CargoResult::Failure => Status::MutantCaught,
        }
    }

    pub fn from_clean_test(cargo_result: &CargoResult) -> Status {
        // TODO: Detect signals and cargo failures other than test failures.
        match cargo_result {
            CargoResult::Timeout => Status::Timeout,
            CargoResult::Success => Status::CleanTestPassed,
            CargoResult::Failure => Status::CleanTestFailed,
        }
    }

    pub fn from_source_build(cargo_result: &CargoResult) -> Status {
        match cargo_result {
            CargoResult::Timeout => Status::Timeout,
            CargoResult::Success => Status::SourceBuildPassed,
            CargoResult::Failure => Status::SourceBuildFailed,
        }
    }

    /// True if this status indicates the user definitely needs to see the logs, because a task
    /// failed that should not have.
    pub fn should_show_logs(&self) -> bool {
        use Status::*;
        matches!(self, CleanTestFailed | SourceBuildFailed)
    }

    /// True if the scenario succeeded.
    pub fn passed(&self) -> bool {
        use Status::*;
        match self {
            MutantCaught | CheckPassed | CleanTestPassed | SourceBuildPassed => true,
            MutantMissed | CheckFailed | CleanTestFailed | SourceBuildFailed | Timeout
            | BuildFailed => false,
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
        if self.count(CleanTestFailed) > 0 || self.count(SourceBuildFailed) > 4 {
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

/// Build tests in the original source tree.
///
/// This brings the source `target` directory basically up to date with any changes to the source,
/// dependencies, or the Rust toolchain. We do this in the source so that repeated runs of `cargo
/// mutants` won't have to repeat this work in every scratch directory.
fn build_source_tree(
    source_tree: &SourceTree,
    output_dir: &OutputDir,
    options: &ExperimentOptions,
    console: &Console,
) -> Result<()> {
    let scenario_name = if options.check_only {
        "check source tree"
    } else {
        "build source tree"
    };
    let mut activity = console.start_activity(scenario_name);
    let mut log_file = output_dir.create_log(scenario_name)?;
    log_file.message(scenario_name);
    let start = Instant::now();

    activity.set_phase("check");
    let test_result = run_cargo(
        &["check", "--tests"],
        source_tree.root(),
        &mut activity,
        &mut log_file,
    )?;
    if !test_result.success() {
        activity.outcome(&Outcome::new(&log_file, &start, Status::SourceBuildFailed))?;
        return Err(anyhow!("check failed in source tree, not continuing"));
    }
    if options.check_only {
        activity.outcome(&Outcome::new(&log_file, &start, Status::CheckPassed))?;
        return Ok(());
    }

    activity.set_phase("build");
    let test_result = run_cargo(
        &["build", "--tests"],
        source_tree.root(),
        &mut activity,
        &mut log_file,
    )?;
    let status = Status::from_source_build(&test_result);
    let outcome = Outcome::new(&log_file, &start, status);
    activity.outcome(&outcome)?;
    if test_result.success() {
        Ok(())
    } else {
        Err(anyhow!("build failed in source tree, not continuing"))
    }
}

/// Test building the unmodified source.
///
/// If there are already-failing tests, proceeding to test mutations
/// won't give a clear signal.
fn test_clean(
    build_dir: &Path,
    output_dir: &OutputDir,
    options: &ExperimentOptions,
    console: &Console,
) -> Result<Outcome> {
    let mut activity = console.start_activity("baseline test with no mutations");
    let scenario_name = "baseline";
    let mut log_file = output_dir.create_log(scenario_name)?;
    log_file.message(scenario_name);
    let outcome = run_scenario(build_dir, &mut activity, &mut log_file, options, true)?;
    activity.outcome(&outcome)?;
    Ok(outcome)
}

/// Test with one mutation applied.
fn test_mutation(
    mutation: &Mutation,
    build_dir: &Path,
    output_dir: &OutputDir,
    options: &ExperimentOptions,
    console: &Console,
) -> Result<Outcome> {
    let mut activity = console.start_mutation(mutation);
    let scenario_name = mutation.to_string();
    let mut log_file = output_dir.create_log(&scenario_name)?;
    log_file.message(&format!("{}\n{}", scenario_name, mutation.diff()));
    let outcome = mutation.with_mutation_applied(build_dir, || {
        run_scenario(build_dir, &mut activity, &mut log_file, options, false)
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
    log_path: PathBuf,
    pub duration: Duration,
}

impl Outcome {
    pub fn new(log_file: &LogFile, start_time: &Instant, status: Status) -> Outcome {
        Outcome {
            log_path: log_file.path().to_owned(),
            duration: start_time.elapsed(),
            status,
        }
    }

    pub fn get_log_content(&self) -> Result<String> {
        fs::read_to_string(&self.log_path).context("read log file")
    }
}

/// Successively run cargo check, build, test, and return the overall outcome.
fn run_scenario(
    build_dir: &Path,
    activity: &mut Activity,
    log_file: &mut LogFile,
    options: &ExperimentOptions,
    is_clean: bool,
) -> Result<Outcome> {
    // TODO: Maybe separate launching and collecting the result, so
    // that we can run several in parallel.

    let start = Instant::now();

    activity.set_phase("check");
    if !run_cargo(&["check"], build_dir, activity, log_file)?.success() {
        return Ok(Outcome::new(&log_file, &start, Status::CheckFailed));
    }
    if options.check_only {
        return Ok(Outcome::new(&log_file, &start, Status::CheckPassed));
    }

    activity.set_phase("build");
    if !run_cargo(&["build", "--tests"], build_dir, activity, log_file)?.success() {
        return Ok(Outcome::new(&log_file, &start, Status::BuildFailed));
    }

    activity.set_phase("test");
    let test_result = run_cargo(&["test"], build_dir, activity, log_file)?;
    let status = if is_clean {
        Status::from_clean_test(&test_result)
    } else {
        Status::from_mutant_test(&test_result)
    };

    Ok(Outcome::new(&log_file, &start, status))
}

fn copy_source_to_scratch(
    source: &SourceTree,
    tmp_path: &Path,
    console: &Console,
) -> Result<PathBuf> {
    let build_dir = tmp_path.join("build");
    let mut activity =
        console.start_copy_activity("copy source and build products to scratch directory");
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

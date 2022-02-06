// Copyright 2021, 2022 Martin Pool

//! Successively apply mutations to the source code and run cargo to check, build, and test them.

use std::fmt;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use path_slash::PathExt;
use tempfile::TempDir;

use crate::console::{self, Console};
use crate::mutate::Mutation;
use crate::outcome::{LabOutcome, Outcome, Phase};
use crate::output::OutputDir;
use crate::run::run_cargo;
use crate::*;

/// What type of build, check, or test was this?
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum Scenario {
    /// Build in the original source tree.
    SourceTree,
    /// Build in a copy of the source tree but with no mutations applied.
    Baseline,
    /// Build with a mutant applied.
    Mutant(Mutation),
}

impl fmt::Display for Scenario {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Scenario::SourceTree => f.write_str("source tree"),
            Scenario::Baseline => f.write_str("baseline"),
            Scenario::Mutant(mutant) => mutant.fmt(f),
        }
    }
}

impl Scenario {
    pub fn is_mutant(&self) -> bool {
        matches!(self, Scenario::Mutant(_))
    }
}

/// Run all possible mutation experiments.
///
/// Before testing the mutations, the lab checks that the source tree passes its tests with no
/// mutations applied.
pub fn test_unmutated_then_all_mutants(
    source_tree: &SourceTree,
    options: &Options,
    console: &Console,
) -> Result<LabOutcome> {
    let mut lab_outcome = LabOutcome::default();
    let output_dir = OutputDir::new(source_tree.root())?;
    let outcome = check_and_build_source_tree(source_tree, &output_dir, options, console)?;
    lab_outcome.add(&outcome);
    if !outcome.success() {
        console::print_error(&format!(
            "{} failed in source tree, not continuing",
            outcome.phase,
        ));
        return Ok(lab_outcome); // TODO: Maybe should be Err?
    }

    let build_dir = copy_source_to_scratch(source_tree, console)?;
    let outcome = test_baseline(build_dir.path(), &output_dir, options, console)?;
    lab_outcome.add(&outcome);
    if !outcome.success() {
        console::print_error(&format!(
            "cargo {} failed in an unmutated tree, so no mutants were tested",
            outcome.phase,
        ));
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
            build_dir.path(),
            &output_dir,
            options,
            console,
        )?);
    }
    Ok(lab_outcome)
}

/// Successively run cargo check, build, test, and return the overall outcome in a build
/// directory, which might have a mutation applied or not.
///
/// This runs the given phases in order until one fails.
///
/// Return the outcome of the last phase run.
fn run_cargo_phases(
    build_dir: &Path,
    output_dir: &OutputDir,
    options: &Options,
    scenario: Scenario,
    phases: &[Phase],
    console: &Console,
) -> Result<Outcome> {
    // TODO: Maybe separate launching and collecting the result, so
    // that we can run several in parallel.
    let scenario_name = scenario.to_string();
    let mut log_file = output_dir.create_log(&scenario_name)?;
    log_file.message(&scenario_name);
    if let Scenario::Mutant(mutant) = &scenario {
        log_file.message(&mutant.diff());
    }
    let mut activity = match &scenario {
        Scenario::SourceTree => {
            console.start_activity(&format!("{} source tree", phases.last().unwrap()))
        }
        Scenario::Baseline => console.start_activity("unmutated baseline"),
        Scenario::Mutant(mutant) => console.start_mutation(mutant),
    };
    let start_time = Instant::now();

    let mut last_cargo_result = None;
    let mut last_phase = None;
    for &phase in phases {
        last_phase = Some(phase);
        activity.set_phase(phase.name());
        let cargo_args: &[&str] = match phase {
            Phase::Check => &["check", "--tests"],
            Phase::Build => &["build", "--tests"],
            Phase::Test => &["test"],
        };
        let timeout = match phase {
            Phase::Test => options.test_timeout(),
            _ => Duration::MAX,
        };
        let cargo_result = run_cargo(cargo_args, build_dir, &mut activity, &mut log_file, timeout)?;
        last_cargo_result = Some(cargo_result);
        if (phase == Phase::Check && options.check_only) || !cargo_result.success() {
            break;
        }
    }
    let outcome = Outcome::new(
        &log_file,
        &start_time,
        scenario,
        last_cargo_result.unwrap(),
        last_phase.unwrap(),
    );
    activity.outcome(&outcome)?;
    Ok(outcome)
}

fn copy_source_to_scratch(source: &SourceTree, console: &Console) -> Result<TempDir> {
    let temp_dir = TempDir::new()?;
    let mut activity =
        console.start_copy_activity("copy source and build products to scratch directory");
    // I thought we could skip copying /target here, but it turns out that copying
    // it does speed up the first build.
    match cp_r::CopyOptions::new()
        .after_entry_copied(|_path, _ft, stats| {
            activity.bytes_copied(stats.file_bytes);
            // TODO: check was_interrupted.
        })
        .copy_tree(source.root(), &temp_dir.path())
        .context("copy source tree to lab directory")
    {
        Ok(stats) => activity.succeed(stats.file_bytes),
        Err(err) => {
            activity.fail();
            eprintln!(
                "error copying source tree {} to {}: {:?}",
                &source.root().to_slash_lossy(),
                &temp_dir.path().to_slash_lossy(),
                err
            );
            return Err(err);
        }
    }
    Ok(temp_dir)
}

/// Build tests in the original source tree.
///
/// This brings the source `target` directory basically up to date with any changes to the source,
/// dependencies, or the Rust toolchain. We do this in the source so that repeated runs of `cargo
/// mutants` won't have to repeat this work in every scratch directory.
fn check_and_build_source_tree(
    source_tree: &SourceTree,
    output_dir: &OutputDir,
    options: &Options,
    console: &Console,
) -> Result<Outcome> {
    let phases: &'static [Phase] = if options.check_only {
        &[Phase::Check]
    } else {
        &[Phase::Check, Phase::Build]
    };
    run_cargo_phases(
        source_tree.root(),
        output_dir,
        options,
        Scenario::SourceTree,
        phases,
        console,
    )
}

/// Test building the unmodified source.
///
/// If there are already-failing tests, proceeding to test mutations
/// won't give a clear signal.
fn test_baseline(
    build_dir: &Path,
    output_dir: &OutputDir,
    options: &Options,
    console: &Console,
) -> Result<Outcome> {
    run_cargo_phases(
        build_dir,
        output_dir,
        options,
        Scenario::Baseline,
        Phase::ALL,
        console,
    )
}

/// Test with one mutation applied.
fn test_mutation(
    mutation: &Mutation,
    build_dir: &Path,
    output_dir: &OutputDir,
    options: &Options,
    console: &Console,
) -> Result<Outcome> {
    mutation.with_mutation_applied(build_dir, || {
        run_cargo_phases(
            build_dir,
            output_dir,
            options,
            Scenario::Mutant(mutation.clone()),
            Phase::ALL,
            console,
        )
    })
}

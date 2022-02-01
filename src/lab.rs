// Copyright 2021, 2022 Martin Pool

//! Successively apply mutations to the source code and run cargo to check, build, and test them.

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};
use path_slash::PathExt;
use tempfile::TempDir;

use crate::console::{self, Activity, Console};
use crate::log_file::LogFile;
use crate::mutate::Mutation;
use crate::outcome::{LabOutcome, Outcome, Phase, Scenario};
use crate::output::OutputDir;
use crate::run::run_cargo;
use crate::*;

/// Run all possible mutation experiments.
///
/// Before testing the mutations, the lab checks that the source tree passes its tests with no
/// mutations applied.
pub fn test_clean_then_all_mutants(
    source_tree: &SourceTree,
    options: &Options,
    console: &Console,
) -> Result<LabOutcome> {
    let mut lab_outcome = LabOutcome::default();
    let output_dir = OutputDir::new(source_tree.root())?;
    let outcome = check_and_build_source_tree(source_tree, &output_dir, options, console)?;
    lab_outcome.add(&outcome);
    if !outcome.cargo_result.success() {
        console::print_error(&format!(
            "{} failed in source tree, not continuing",
            outcome.phase,
        ));
        return Ok(lab_outcome); // TODO: Maybe should be Err?
    }

    let tmp_dir = TempDir::new()?;
    let build_dir = copy_source_to_scratch(source_tree, tmp_dir.path(), console)?;

    let outcome = test_baseline(&build_dir, &output_dir, options, console)?;
    lab_outcome.add(&outcome);
    if !outcome.cargo_result.success() {
        console::print_error(&format!(
            "{} failed in a clean copy of the tree, so no mutants were tested",
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
            &build_dir,
            &output_dir,
            options,
            console,
        )?);
    }
    Ok(lab_outcome)
}

/// Successively run cargo check, build, test, and return the overall outcome in a build
/// directory, which might have a mutation applied or not.
fn check_build_test_dir(
    build_dir: &Path,
    activity: &mut Activity,
    log_file: &mut LogFile,
    options: &Options,
    scenario: Scenario,
) -> Result<Outcome> {
    // TODO: Maybe separate launching and collecting the result, so
    // that we can run several in parallel.
    let start_time = Instant::now();
    let mut last_outcome = None;
    for phase in [Phase::Check, Phase::Build, Phase::Test] {
        activity.set_phase(&phase.name());
        let cargo_args: &[&str] = match phase {
            Phase::Check => &["check"],
            Phase::Build => &["build", "--tests"],
            Phase::Test => &["test"],
        };
        let cargo_result = run_cargo(cargo_args, build_dir, activity, log_file, options)?;
        let outcome = Outcome::new(&log_file, &start_time, scenario, cargo_result, phase);
        if (phase == Phase::Check && options.check_only) || !cargo_result.success() {
            return Ok(outcome);
        }
        last_outcome = Some(outcome);
    }
    Ok(last_outcome.unwrap())
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
            // TODO: check was_interrupted.
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
    let scenario_name = if options.check_only {
        "check source tree"
    } else {
        "build source tree"
    };
    let scenario = Scenario::SourceTree;
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
        options,
    )?;
    if options.check_only || !test_result.success() {
        let outcome = Outcome::new(&log_file, &start, scenario, test_result, Phase::Check);
        activity.outcome(&outcome)?;
        return Ok(outcome);
    }

    activity.set_phase("build");
    let test_result = run_cargo(
        &["build", "--tests"],
        source_tree.root(),
        &mut activity,
        &mut log_file,
        options,
    )?;
    let outcome = Outcome::new(&log_file, &start, scenario, test_result, Phase::Build);
    activity.outcome(&outcome)?;
    Ok(outcome)
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
    let mut activity = console.start_activity("baseline test with no mutations");
    let scenario_name = "baseline";
    let mut log_file = output_dir.create_log(scenario_name)?;
    log_file.message(scenario_name);
    let outcome = check_build_test_dir(
        build_dir,
        &mut activity,
        &mut log_file,
        options,
        Scenario::Baseline,
    )?;
    activity.outcome(&outcome)?;
    Ok(outcome)
}

/// Test with one mutation applied.
fn test_mutation(
    mutation: &Mutation,
    build_dir: &Path,
    output_dir: &OutputDir,
    options: &Options,
    console: &Console,
) -> Result<Outcome> {
    let mut activity = console.start_mutation(mutation);
    let scenario_name = mutation.to_string();
    let mut log_file = output_dir.create_log(&scenario_name)?;
    log_file.message(&format!("{}\n{}", scenario_name, mutation.diff()));
    let outcome = mutation.with_mutation_applied(build_dir, || {
        check_build_test_dir(
            build_dir,
            &mut activity,
            &mut log_file,
            options,
            Scenario::Mutant,
        )
    })?;
    activity.outcome(&outcome)?;
    Ok(outcome)
}

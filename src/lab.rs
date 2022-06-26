// Copyright 2021, 2022 Martin Pool

//! Successively apply mutations to the source code and run cargo to check, build, and test them.

use std::cmp::max;
use std::fmt;
use std::fs::File;
use std::io::BufWriter;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use camino::Utf8Path;
use rand::prelude::*;
use serde::Serialize;

use crate::cargo::run_cargo;
use crate::console::{self, LabActivity};
use crate::mutate::Mutant;
use crate::outcome::{LabOutcome, Outcome, Phase};
use crate::output::OutputDir;
use crate::*;

/// What type of build, check, or test was this?
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub enum Scenario {
    /// Build in the original source tree.
    SourceTree,
    /// Build in a copy of the source tree but with no mutations applied.
    Baseline,
    /// Build with a mutation applied.
    Mutant(Mutant),
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
        matches!(self, Scenario::Mutant { .. })
    }

    pub(crate) fn log_file_name_base(&self) -> String {
        match self {
            Scenario::SourceTree => "source_tree".into(),
            Scenario::Baseline => "baseline".into(),
            Scenario::Mutant(mutant) => mutant.log_file_name_base(),
        }
    }
}

/// Run all possible mutation experiments.
///
/// Before testing the mutants, the lab checks that the source tree passes its tests with no
/// mutations applied.
pub fn test_unmutated_then_all_mutants(
    source_tree: &SourceTree,
    options: &Options,
) -> Result<LabOutcome> {
    let mut options: Options = options.clone();
    let mut lab_outcome = LabOutcome::default();
    let output_in_dir = if let Some(o) = &options.output_in_dir {
        o.as_path()
    } else {
        source_tree.path()
    };
    let output_dir = OutputDir::new(output_in_dir)?;
    let mut lab_activity = LabActivity::new(&options);

    if options.build_source {
        let outcome =
            check_and_build_source_tree(source_tree, &output_dir, &options, &mut lab_activity)?;
        lab_outcome.add(&outcome);
        if !outcome.success() {
            console::print_error(&format!(
                "cargo {} failed in source tree, not continuing",
                outcome.last_phase(),
            ));
            return Ok(lab_outcome); // TODO: Maybe should be Err?
        }
    }

    let build_dir = BuildDir::new(source_tree, &options)?;
    let build_dir_path = build_dir.path();
    let outcome = {
        run_cargo_phases(
            build_dir_path,
            &output_dir,
            &options,
            &Scenario::Baseline,
            Phase::ALL,
            &mut lab_activity,
        )
    }?;
    lab_outcome.add(&outcome);
    if !outcome.success() {
        console::print_error(&format!(
            "cargo {} failed in an unmutated tree, so no mutants were tested",
            outcome.last_phase(),
        ));
        return Ok(lab_outcome); // TODO: Maybe should be Err?
    }
    if !options.has_test_timeout() {
        if let Some(baseline_duration) = outcome.test_duration() {
            let auto_timeout = max(Duration::from_secs(20), baseline_duration.mul_f32(5.0));
            options.set_test_timeout(auto_timeout);
            if options.show_times {
                println!(
                    "Auto-set test timeout to {:.1}s",
                    options.test_timeout().as_secs_f32()
                );
            }
        }
    }

    let mut mutants = source_tree.mutants(&options)?;
    if options.shuffle {
        mutants.shuffle(&mut rand::thread_rng());
    }

    serde_json::to_writer_pretty(
        BufWriter::new(File::create(output_dir.path().join("mutants.json"))?),
        &mutants,
    )?;
    println!(
        "Found {} {} to test",
        mutants.len(),
        if mutants.len() == 1 {
            "mutant"
        } else {
            "mutants"
        }
    );
    if mutants.is_empty() {
        return Err(anyhow!("No mutants found"));
    }

    lab_activity.start_mutants(mutants.len());
    for mutant in mutants {
        let scenario = Scenario::Mutant(mutant.clone());
        let outcome = mutant.with_mutation_applied(&build_dir, || {
            run_cargo_phases(
                build_dir_path,
                &output_dir,
                &options,
                &scenario,
                Phase::ALL,
                &mut lab_activity,
            )
        })?;
        lab_outcome.add(&outcome);

        // Rewrite outcomes.json every time, so we can watch it and so it's not
        // lost if the program stops or is interrupted.
        serde_json::to_writer_pretty(
            BufWriter::new(File::create(output_dir.path().join("outcomes.json"))?),
            &lab_outcome,
        )?;
    }
    Ok(lab_outcome)
}

/// Successively run cargo check, build, test, and return the overall outcome in a build
/// directory, which might have a mutation applied or not.
///
/// This runs the given phases in order until one fails.
///
/// `in_dir` may be the path of either a source tree (for freshening) or a
/// [BuildDir] (for baseline and mutation builds.)
///
/// Return the outcome of the last phase run.
fn run_cargo_phases(
    in_dir: &Utf8Path,
    output_dir: &OutputDir,
    options: &Options,
    scenario: &Scenario,
    phases: &[Phase],
    lab_activity: &mut LabActivity,
) -> Result<Outcome> {
    let mut log_file = output_dir.create_log(scenario)?;
    log_file.message(&scenario.to_string());
    if let Scenario::Mutant(mutant) = scenario {
        log_file.message(&mutant.diff());
    }
    let mut cargo_activity = lab_activity.start_scenario(scenario, log_file.path().to_owned());

    let mut outcome = Outcome::new(&log_file, scenario.clone());
    for &phase in phases {
        let phase_start = Instant::now();
        cargo_activity.set_phase(phase.name());
        let cargo_args = match phase {
            Phase::Check => vec!["check", "--tests"],
            Phase::Build => vec!["build", "--tests"],
            Phase::Test => std::iter::once("test")
                .chain(
                    options
                        .additional_cargo_test_args
                        .iter()
                        .map(String::as_str),
                )
                .collect(),
        };
        let timeout = match phase {
            Phase::Test => options.test_timeout(),
            _ => Duration::MAX,
        };
        let cargo_result = run_cargo(
            &cargo_args,
            in_dir,
            &mut cargo_activity,
            &mut log_file,
            timeout,
        )?;
        outcome.add_phase_result(phase, phase_start.elapsed(), cargo_result);
        if (phase == Phase::Check && options.check_only) || !cargo_result.success() {
            break;
        }
    }
    cargo_activity.outcome(&outcome, options)?;
    Ok(outcome)
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
    lab_activity: &mut LabActivity,
) -> Result<Outcome> {
    let phases: &'static [Phase] = if options.check_only {
        &[Phase::Check]
    } else {
        &[Phase::Check, Phase::Build]
    };
    run_cargo_phases(
        source_tree.path(),
        output_dir,
        options,
        &Scenario::SourceTree,
        phases,
        lab_activity,
    )
}

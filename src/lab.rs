// Copyright 2021, 2022 Martin Pool

//! Successively apply mutations to the source code and run cargo to check, build, and test them.

use std::cmp::max;
use std::fs::File;
use std::io::BufWriter;

use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use camino::Utf8Path;
use rand::prelude::*;
use tracing::error;
#[allow(unused_imports)]
use tracing::{debug, info};

use tracing_subscriber::prelude::*;

use crate::cargo::{cargo_argv, run_cargo};
use crate::console::{plural, Console};
use crate::outcome::{LabOutcome, Outcome, Phase};
use crate::output::OutputDir;
use crate::*;

/// Run all possible mutation experiments.
///
/// Before testing the mutants, the lab checks that the source tree passes its tests with no
/// mutations applied.
pub fn test_unmutated_then_all_mutants(
    source_tree: &SourceTree,
    mut options: Options,
    console_trace_level: tracing::Level,
) -> Result<LabOutcome> {
    let start_time = Instant::now();
    let output_in_dir = if let Some(o) = &options.output_in_dir {
        o.as_path()
    } else {
        source_tree.path()
    };
    let output_dir = OutputDir::new(output_in_dir)?;

    let console = Console::new();

    console.set_debug_log(output_dir.open_debug_log()?);
    let debug_log_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_file(true) // source file name
        .with_line_number(true)
        .with_writer(console.make_debug_log_writer());
    let level_filter = tracing_subscriber::filter::LevelFilter::from_level(console_trace_level);
    let console_layer = tracing_subscriber::fmt::layer()
        .with_ansi(true)
        .with_writer(console.make_terminal_writer())
        .with_target(false)
        .without_time()
        .with_filter(level_filter);
    tracing_subscriber::registry()
        .with(debug_log_layer)
        .with(console_layer)
        .init();

    let mut lab_outcome = LabOutcome::default();
    if options.build_source {
        let outcome = build_source_tree(source_tree, &output_dir, &options, &console)?;
        lab_outcome.add(&outcome);
        output_dir.write_outcomes_json(&lab_outcome)?;
        if !outcome.success() {
            error!(
                "cargo {} failed in source tree, not continuing",
                outcome.last_phase(),
            );
            return Ok(lab_outcome); // TODO: Maybe should be Err?
        }
    }

    let build_dir = BuildDir::new(source_tree, &options)?;
    let build_dir_path = build_dir.path();
    let phases: &[Phase] = if options.check_only {
        &[Phase::Check]
    } else {
        &[Phase::Build, Phase::Test]
    };
    let outcome = {
        run_cargo_phases(
            build_dir_path,
            &output_dir,
            &options,
            &Scenario::Baseline,
            phases,
            &console,
        )
    }?;
    lab_outcome.add(&outcome);
    output_dir.write_outcomes_json(&lab_outcome)?;
    if !outcome.success() {
        error!(
            "cargo {} failed in an unmutated tree, so no mutants were tested",
            outcome.last_phase(),
        );
        return Ok(lab_outcome); // TODO: Maybe should be Err?
    }
    if !options.has_test_timeout() {
        if let Some(baseline_duration) = outcome.test_duration() {
            let auto_timeout = max(minimum_test_timeout()?, baseline_duration.mul_f32(5.0));
            options.set_test_timeout(auto_timeout);
            if options.show_times {
                console.autoset_timeout(auto_timeout);
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
    println!("Found {} to test", plural(mutants.len(), "mutant"));
    if mutants.is_empty() {
        return Err(anyhow!("No mutants found"));
    }

    console.start_testing_mutants(mutants.len());
    for mutant in mutants {
        let scenario = Scenario::Mutant(mutant.clone());
        let outcome = mutant.with_mutation_applied(&build_dir, || {
            run_cargo_phases(
                build_dir_path,
                &output_dir,
                &options,
                &scenario,
                phases,
                &console,
            )
        })?;
        lab_outcome.add(&outcome);
        // Rewrite outcomes.json every time, so we can watch it and so it's not
        // lost if the program stops or is interrupted.
        output_dir.write_outcomes_json(&lab_outcome)?;
    }
    console.message(&format!(
        "{}\n",
        lab_outcome.summary_string(start_time, &options)
    ));
    Ok(lab_outcome)
}

/// Return the minimum timeout for cargo tests (used if the baseline tests are fast),
/// from either the environment or a built-in default.
fn minimum_test_timeout() -> Result<Duration> {
    let var_name = crate::MINIMUM_TEST_TIMEOUT_ENV_VAR;
    if let Some(env_timeout) = env::var_os(var_name) {
        let env_timeout = env_timeout
            .to_string_lossy()
            .parse()
            .with_context(|| format!("invalid {var_name}"))?;
        Ok(Duration::from_secs(env_timeout))
    } else {
        Ok(DEFAULT_MINIMUM_TEST_TIMEOUT)
    }
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
    console: &Console,
) -> Result<Outcome> {
    debug!("start testing {scenario} in {in_dir}");
    let mut log_file = output_dir.create_log(scenario)?;
    log_file.message(&scenario.to_string());
    if let Scenario::Mutant(mutant) = scenario {
        log_file.message(&format!("mutation diff:\n{}", mutant.diff()));
    }
    console.scenario_started(scenario, log_file.path());

    let mut outcome = Outcome::new(&log_file, scenario.clone());
    for &phase in phases {
        let phase_start = Instant::now();
        console.scenario_phase_started(phase);
        let cargo_argv = cargo_argv(scenario.package_name(), phase, options);
        let timeout = match phase {
            Phase::Test => options.test_timeout(),
            _ => Duration::MAX,
        };
        let cargo_result = run_cargo(&cargo_argv, in_dir, &mut log_file, timeout, console)?;
        outcome.add_phase_result(phase, phase_start.elapsed(), cargo_result, &cargo_argv);
        console.scenario_phase_finished(phase);
        if (phase == Phase::Check && options.check_only) || !cargo_result.success() {
            break;
        }
    }
    debug!("{scenario} outcome {:?}", outcome.summary());
    console.scenario_finished(scenario, &outcome, options);

    Ok(outcome)
}

/// Build tests in the original source tree.
///
/// This brings the source `target` directory basically up to date with any changes to the source,
/// dependencies, or the Rust toolchain. We do this in the source so that repeated runs of `cargo
/// mutants` won't have to repeat this work in every scratch directory.
fn build_source_tree(
    source_tree: &SourceTree,
    output_dir: &OutputDir,
    options: &Options,
    console: &Console,
) -> Result<Outcome> {
    let phases: &'static [Phase] = if options.check_only {
        &[Phase::Check]
    } else {
        &[Phase::Build]
    };
    run_cargo_phases(
        source_tree.path(),
        output_dir,
        options,
        &Scenario::SourceTree,
        phases,
        console,
    )
}

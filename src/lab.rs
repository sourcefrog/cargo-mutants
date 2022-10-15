// Copyright 2021, 2022 Martin Pool

//! Successively apply mutations to the source code and run cargo to check, build, and test them.

use std::cmp::max;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use rand::prelude::*;
#[allow(unused)]
use tracing::{debug, debug_span, error, info};

use crate::cargo::{cargo_argv, run_cargo, CargoSourceTree};
use crate::console::Console;
use crate::outcome::{LabOutcome, Phase, ScenarioOutcome};
use crate::output::OutputDir;
use crate::*;

/// Run all possible mutation experiments.
///
/// Before testing the mutants, the lab checks that the source tree passes its tests with no
/// mutations applied.
pub fn test_unmutated_then_all_mutants(
    source_tree: &CargoSourceTree,
    options: Options,
    console: &Console,
) -> Result<LabOutcome> {
    let start_time = Instant::now();
    let output_in_dir = if let Some(o) = &options.output_in_dir {
        o.as_path()
    } else {
        source_tree.path()
    };
    let mut output_dir = OutputDir::new(output_in_dir)?;
    console.set_debug_log(output_dir.open_debug_log()?);
    let mut lab_outcome = LabOutcome::new();

    let mut mutants = source_tree.mutants(&options)?;
    if options.shuffle {
        mutants.shuffle(&mut rand::thread_rng());
    }
    output_dir.write_mutants_list(&mutants)?;
    console.discovered_mutants(&mutants);
    if mutants.is_empty() {
        return Err(anyhow!("No mutants found"));
    }

    let mut build_dirs = vec![BuildDir::new(source_tree, console)?];
    let phases: &[Phase] = if options.check_only {
        &[Phase::Check]
    } else {
        &[Phase::Build, Phase::Test]
    };
    let baseline_outcome = {
        let _span = debug_span!("baseline").entered();
        test_scenario(
            &mut build_dirs[0],
            &mut output_dir,
            &options,
            &Scenario::Baseline,
            phases,
            options.test_timeout.unwrap_or(Duration::MAX),
            console,
        )?
    };
    lab_outcome.add(baseline_outcome.clone());
    output_dir.update_lab_outcome(&lab_outcome)?;
    if !baseline_outcome.success() {
        error!(
            "cargo {} failed in an unmutated tree, so no mutants were tested",
            baseline_outcome.last_phase(),
        );
        return Ok(lab_outcome); // TODO: Maybe should be Err?
    }

    let mutated_test_timeout = if let Some(timeout) = options.test_timeout {
        timeout
    } else if let Some(baseline_test_duration) = baseline_outcome.test_duration() {
        // If we didn't run tests in the baseline, e.g. for `--check`, there might be no duration.
        let auto_timeout = max(minimum_test_timeout()?, baseline_test_duration.mul_f32(5.0));
        if options.show_times {
            console.autoset_timeout(auto_timeout);
        }
        auto_timeout
    } else {
        Duration::MAX
    };

    // build_dirs.push(build_dirs[0].copy(console)?);
    console.start_testing_mutants(mutants.len());
    for (mutant_id, mutant) in mutants.into_iter().enumerate() {
        let _span = debug_span!("mutant", id = mutant_id).entered();
        debug!(location = %mutant.describe_location(), change = ?mutant.describe_change());
        let outcome = test_scenario(
            &mut build_dirs[0],
            &mut output_dir,
            &options,
            &Scenario::Mutant(mutant),
            phases,
            mutated_test_timeout,
            console,
        )?;
        lab_outcome.add(outcome);
        // Rewrite outcomes.json every time, so we can watch it and so it's not
        // lost if the program stops or is interrupted.
        output_dir.update_lab_outcome(&lab_outcome)?;
    }
    console.lab_finished(&lab_outcome, start_time, &options);
    Ok(lab_outcome)
}

/// Test various phases of one scenario in a build dir.
///
/// This runs the given phases in order until one fails.
///
/// The [BuildDir] is passed as mutable because it's for the exclusive use of this function for the
/// duration of the test.
fn test_scenario(
    build_dir: &mut BuildDir,
    output_dir: &mut OutputDir,
    options: &Options,
    scenario: &Scenario,
    phases: &[Phase],
    test_timeout: Duration,
    console: &Console,
) -> Result<ScenarioOutcome> {
    let mut log_file = output_dir.create_log(scenario)?;
    log_file.message(&scenario.to_string());
    if let Scenario::Mutant(mutant) = scenario {
        log_file.message(&format!("mutation diff:\n{}", mutant.diff()));
        mutant.apply(build_dir)?;
    }
    console.scenario_started(scenario, log_file.path());

    let mut outcome = ScenarioOutcome::new(&log_file, scenario.clone());
    for &phase in phases {
        let phase_start = Instant::now();
        console.scenario_phase_started(phase);
        let cargo_argv = cargo_argv(scenario.package_name(), phase, options);
        let timeout = match phase {
            Phase::Test => test_timeout,
            _ => Duration::MAX,
        };
        let cargo_result = run_cargo(
            &cargo_argv,
            build_dir.path(),
            &mut log_file,
            timeout,
            console,
        )?;
        outcome.add_phase_result(phase, phase_start.elapsed(), cargo_result, &cargo_argv);
        console.scenario_phase_finished(phase);
        if (phase == Phase::Check && options.check_only) || !cargo_result.success() {
            break;
        }
    }
    if let Scenario::Mutant(mutant) = scenario {
        mutant.unapply(build_dir)?;
    }
    output_dir.add_scenario_outcome(&outcome)?;
    debug!(outcome = ?outcome.summary());
    console.scenario_finished(scenario, &outcome, options);

    Ok(outcome)
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

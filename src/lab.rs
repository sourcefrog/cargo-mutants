// Copyright 2021, 2022 Martin Pool

//! Successively apply mutations to the source code and run cargo to check, build, and test them.

use std::cmp::max;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use rand::prelude::*;
#[allow(unused)]
use tracing::{debug, debug_span, error, info, trace};

use crate::cargo::{cargo_argv, run_cargo, rustflags, CargoSourceTree};
use crate::console::Console;
use crate::outcome::{LabOutcome, Phase, ScenarioOutcome};
use crate::output::OutputDir;
use crate::visit::discover_mutants;
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
    let output_dir = OutputDir::new(output_in_dir)?;
    console.set_debug_log(output_dir.open_debug_log()?);

    let rustflags = rustflags();
    let mut mutants = discover_mutants(source_tree, &options)?;
    if options.shuffle {
        mutants.shuffle(&mut rand::thread_rng());
    }
    output_dir.write_mutants_list(&mutants)?;
    console.discovered_mutants(&mutants);
    if mutants.is_empty() {
        return Err(anyhow!("No mutants found"));
    }

    let output_mutex = Mutex::new(output_dir);
    let mut build_dirs = vec![BuildDir::new(source_tree, console)?];
    let baseline_outcome = {
        let _span = debug_span!("baseline").entered();
        test_scenario(
            &mut build_dirs[0],
            &output_mutex,
            &options,
            &Scenario::Baseline,
            options.test_timeout.unwrap_or(Duration::MAX),
            console,
            &rustflags,
        )?
    };
    if !baseline_outcome.success() {
        error!(
            "cargo {} failed in an unmutated tree, so no mutants were tested",
            baseline_outcome.last_phase(),
        );
        // TODO: Maybe should be Err, but it would need to be an error that can map to the right
        // exit code.
        return Ok(output_mutex
            .into_inner()
            .expect("lock output_dir")
            .take_lab_outcome());
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

    let jobs = std::cmp::max(1, std::cmp::min(options.jobs.unwrap_or(1), mutants.len()));
    // Create more build dirs
    // TODO: Progress indicator; maybe run them in parallel.
    console.build_dirs_start(jobs - 1);
    for i in 1..jobs {
        debug!("copy build dir {i}");
        build_dirs.push(build_dirs[0].copy(console).context("copy build dir")?);
    }
    console.build_dirs_finished();
    debug!(build_dirs = ?build_dirs);

    // Create n threads, each dedicated to one build directory. Each of them tries to take a
    // scenario to test off the queue, and then exits when there are no more left.
    console.start_testing_mutants(mutants.len());
    let numbered_mutants = Mutex::new(mutants.into_iter().enumerate());
    thread::scope(|scope| {
        let mut threads = Vec::new();
        for build_dir in build_dirs {
            threads.push(scope.spawn(|| {
                let mut build_dir = build_dir; // move it into this thread
                let _thread_span = debug_span!("test thread", thread = ?thread::current().id()).entered();
                trace!("start thread in {build_dir:?}");
                loop {
                    // Not a while loop so that it only holds the lock briefly.
                    let next = numbered_mutants.lock().expect("lock mutants queue").next();
                    if let Some((mutant_id, mutant)) = next {
                        let _span = debug_span!("mutant", id = mutant_id).entered();
                        debug!(location = %mutant.describe_location(), change = ?mutant.describe_change());
                        // We don't care about the outcome; it's been collected into the output_dir.
                        let _outcome = test_scenario(
                            &mut build_dir,
                            &output_mutex,
                            &options,
                            &Scenario::Mutant(mutant),
                            mutated_test_timeout,
                            console,
                            &rustflags,
                        )
                        .expect("scenario test");
                    } else {
                        trace!("no more work");
                        break
                    }
                }
            }));
        }
        for thread in threads {
            thread.join().expect("join thread");
        }
    });

    let output_dir = output_mutex
        .into_inner()
        .expect("final unlock mutants queue");
    console.lab_finished(&output_dir.lab_outcome, start_time, &options);
    Ok(output_dir.take_lab_outcome())
}

/// Test various phases of one scenario in a build dir.
///
/// The [BuildDir] is passed as mutable because it's for the exclusive use of this function for the
/// duration of the test.
fn test_scenario(
    build_dir: &mut BuildDir,
    output_mutex: &Mutex<OutputDir>,
    options: &Options,
    scenario: &Scenario,
    test_timeout: Duration,
    console: &Console,
    rustflags: &str,
) -> Result<ScenarioOutcome> {
    let mut log_file = output_mutex
        .lock()
        .expect("lock output_dir to create log")
        .create_log(scenario)?;
    log_file.message(&scenario.to_string());
    if let Scenario::Mutant(mutant) = scenario {
        log_file.message(&format!("mutation diff:\n{}", mutant.diff()));
        mutant.apply(build_dir)?;
    }
    console.scenario_started(scenario, log_file.path());
    let diff_filename = output_mutex.lock().unwrap().write_diff_file(scenario)?;

    let mut outcome = ScenarioOutcome::new(&log_file, diff_filename, scenario.clone());
    let phases: &[Phase] = if options.check_only {
        &[Phase::Check]
    } else {
        &[Phase::Build, Phase::Test]
    };
    for &phase in phases {
        let phase_start = Instant::now();
        console.scenario_phase_started(scenario, phase);
        let cargo_argv = cargo_argv(scenario.package_name(), phase, options);
        let timeout = match phase {
            Phase::Test => test_timeout,
            _ => Duration::MAX,
        };
        let cargo_result = run_cargo(
            build_dir,
            &cargo_argv,
            &mut log_file,
            timeout,
            console,
            rustflags,
        )?;
        outcome.add_phase_result(phase, phase_start.elapsed(), cargo_result, &cargo_argv);
        console.scenario_phase_finished(scenario, phase);
        if (phase == Phase::Check && options.check_only) || !cargo_result.success() {
            break;
        }
    }
    if let Scenario::Mutant(mutant) = scenario {
        mutant.unapply(build_dir)?;
    }
    output_mutex
        .lock()
        .expect("lock output dir to add outcome")
        .add_scenario_outcome(&outcome)?;
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

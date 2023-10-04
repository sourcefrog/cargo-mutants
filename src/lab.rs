// Copyright 2021-2023 Martin Pool

//! Successively apply mutations to the source code and run cargo to check, build, and test them.

use std::cmp::max;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use itertools::Itertools;
use tracing::warn;
#[allow(unused)]
use tracing::{debug, debug_span, error, info, trace};

use crate::console::Console;
use crate::outcome::{LabOutcome, Phase, PhaseResult, ScenarioOutcome};
use crate::output::OutputDir;
use crate::process::Process;
use crate::source::Package;
use crate::*;

/// Run all possible mutation experiments.
///
/// Before testing the mutants, the lab checks that the source tree passes its tests with no
/// mutations applied.
pub fn test_unmutated_then_all_mutants(
    tool: &dyn Tool,
    source_tree: &Utf8Path,
    options: Options,
    console: &Console,
) -> Result<LabOutcome> {
    let start_time = Instant::now();
    let output_in_dir: &Utf8Path = options
        .output_in_dir
        .as_ref()
        .map_or(source_tree, |p| p.as_path());
    let output_dir = OutputDir::new(output_in_dir)?;
    console.set_debug_log(output_dir.open_debug_log()?);

    let mut mutants = walk_tree(tool, source_tree, &options, console)?.mutants;
    if options.shuffle {
        fastrand::shuffle(&mut mutants);
    }
    output_dir.write_mutants_list(&mutants)?;
    console.discovered_mutants(&mutants);
    if mutants.is_empty() {
        return Err(anyhow!("No mutants found"));
    }
    let all_packages = mutants.iter().map(|m| m.package()).unique().collect_vec();

    let output_mutex = Mutex::new(output_dir);
    let mut build_dirs = vec![BuildDir::new(source_tree, &options, console)?];
    let baseline_outcome = {
        let _span = debug_span!("baseline").entered();
        test_scenario(
            tool,
            &mut build_dirs[0],
            &output_mutex,
            &options,
            &Scenario::Baseline,
            &all_packages,
            options.test_timeout.unwrap_or(Duration::MAX),
            console,
        )?
    };
    if !baseline_outcome.success() {
        error!(
            "{} {} failed in an unmutated tree, so no mutants were tested",
            tool.name(),
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
    } else if let Some(baseline_test_duration) = baseline_outcome
        .phase_results()
        .iter()
        .find(|r| r.phase == Phase::Test)
        .map(|r| r.duration)
    {
        let auto_timeout = max(
            options.minimum_test_timeout,
            baseline_test_duration.mul_f32(5.0),
        );
        if options.show_times {
            console.autoset_timeout(auto_timeout);
        }
        auto_timeout
    } else {
        Duration::MAX
    };

    let jobs = std::cmp::max(1, std::cmp::min(options.jobs.unwrap_or(1), mutants.len()));
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
                let _thread_span =
                    debug_span!("test thread", thread = ?thread::current().id()).entered();
                trace!("start thread in {build_dir:?}");
                loop {
                    // Not a while loop so that it only holds the lock briefly.
                    let next = numbered_mutants.lock().expect("lock mutants queue").next();
                    if let Some((mutant_id, mutant)) = next {
                        let _span = debug_span!("mutant", id = mutant_id).entered();
                        let package = mutant.package().clone();
                        // We don't care about the outcome; it's been collected into the output_dir.
                        let _outcome = test_scenario(
                            tool,
                            &mut build_dir,
                            &output_mutex,
                            &options,
                            &Scenario::Mutant(mutant),
                            &[&package],
                            mutated_test_timeout,
                            console,
                        )
                        .expect("scenario test");
                    } else {
                        trace!("no more work");
                        break;
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
    let lab_outcome = output_dir.take_lab_outcome();
    if lab_outcome.total_mutants == 0 {
        // This should be unreachable as we also bail out before copying
        // the tree if no mutants are generated.
        warn!("No mutants were generated");
    } else if lab_outcome.unviable == lab_outcome.total_mutants {
        warn!("No mutants were viable; perhaps there is a problem with building in a scratch directory");
    }
    Ok(lab_outcome)
}

/// Test various phases of one scenario in a build dir.
///
/// The [BuildDir] is passed as mutable because it's for the exclusive use of this function for the
/// duration of the test.
#[allow(unknown_lints, clippy::needless_pass_by_ref_mut)]
fn test_scenario(
    tool: &dyn Tool,
    build_dir: &mut BuildDir,
    output_mutex: &Mutex<OutputDir>,
    options: &Options,
    scenario: &Scenario,
    packages: &[&Package],
    test_timeout: Duration,
    console: &Console,
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

    let mut outcome = ScenarioOutcome::new(&log_file, scenario.clone());
    let phases: &[Phase] = if options.check_only {
        &[Phase::Check]
    } else {
        &[Phase::Build, Phase::Test]
    };
    for &phase in phases {
        let _span = debug_span!("run", ?phase).entered();
        let start = Instant::now();
        console.scenario_phase_started(scenario, phase);
        let timeout = match phase {
            Phase::Test => test_timeout,
            _ => Duration::MAX,
        };
        let argv = tool.compose_argv(build_dir, Some(packages), phase, options)?;
        let env = tool.compose_env()?;
        let process_status = Process::run(
            &argv,
            &env,
            build_dir.path(),
            timeout,
            &mut log_file,
            console,
        )?;
        check_interrupted()?;
        debug!(?process_status, elapsed = ?start.elapsed());
        let phase_result = PhaseResult {
            phase,
            duration: start.elapsed(),
            process_status,
            argv,
        };
        outcome.add_phase_result(phase_result);
        console.scenario_phase_finished(scenario, phase);
        if (phase == Phase::Check && options.check_only) || !process_status.success() {
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

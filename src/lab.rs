// Copyright 2021-2023 Martin Pool

//! Successively apply mutations to the source code and run cargo to check, build, and test them.

use std::cmp::{max, min};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use itertools::Itertools;
use tracing::warn;
#[allow(unused)]
use tracing::{debug, debug_span, error, info, trace};

use crate::cargo::run_cargo;
use crate::console::Console;
use crate::outcome::{LabOutcome, Phase, ScenarioOutcome};
use crate::output::OutputDir;
use crate::package::Package;
use crate::*;

/// Run all possible mutation experiments.
///
/// This is called after all filtering is complete, so all the mutants here will be tested
/// or checked.
///
/// Before testing the mutants, the lab checks that the source tree passes its tests with no
/// mutations applied.
pub fn test_mutants(
    mut mutants: Vec<Mutant>,
    workspace_dir: &Utf8Path,
    options: Options,
    console: &Console,
) -> Result<LabOutcome> {
    let start_time = Instant::now();
    let output_in_dir: &Utf8Path = options
        .output_in_dir
        .as_ref()
        .map_or(workspace_dir, |p| p.as_path());
    let output_dir = OutputDir::new(output_in_dir)?;
    console.set_debug_log(output_dir.open_debug_log()?);

    if options.shuffle {
        fastrand::shuffle(&mut mutants);
    }
    output_dir.write_mutants_list(&mutants)?;
    console.discovered_mutants(&mutants);
    if mutants.is_empty() {
        warn!("No mutants found under the active filters");
        return Ok(LabOutcome::default());
    }
    let all_packages = mutants.iter().map(|m| m.package()).unique().collect_vec();
    debug!(?all_packages);

    let output_mutex = Mutex::new(output_dir);
    let build_dir = if options.in_place {
        BuildDir::in_place(workspace_dir)?
    } else {
        BuildDir::copy_from(workspace_dir, options.gitignore, options.leak_dirs, console)?
    };
    let baseline_outcome = match options.baseline {
        BaselineStrategy::Run => {
            let outcome = test_scenario(
                &build_dir,
                &output_mutex,
                &Scenario::Baseline,
                &all_packages,
                options.test_timeout.unwrap_or(Duration::MAX),
                &options,
                console,
            )?;
            if !outcome.success() {
                error!(
                    "cargo {} failed in an unmutated tree, so no mutants were tested",
                    outcome.last_phase(),
                );
                // We "successfully" established that the baseline tree doesn't work; arguably this should be represented as an error
                // but we'd need a way for that error to convey an exit code...
                return Ok(output_mutex
                    .into_inner()
                    .expect("lock output_dir")
                    .take_lab_outcome());
            }
            Some(outcome)
        }
        BaselineStrategy::Skip => None,
    };
    let mut build_dirs = vec![build_dir];
    let test_timeout = test_timeout(&baseline_outcome, &options, console);

    let jobs = max(1, min(options.jobs.unwrap_or(1), mutants.len()));
    console.build_dirs_start(jobs - 1);
    for i in 1..jobs {
        debug!("copy build dir {i}");
        build_dirs.push(BuildDir::copy_from(
            workspace_dir,
            options.gitignore,
            options.leak_dirs,
            console,
        )?);
    }
    console.build_dirs_finished();
    debug!(build_dirs = ?build_dirs);

    // Create n threads, each dedicated to one build directory. Each of them tries to take a
    // scenario to test off the queue, and then exits when there are no more left.
    console.start_testing_mutants(mutants.len());
    let numbered_mutants = Mutex::new(mutants.into_iter().enumerate());
    thread::scope(|scope| {
        let mut threads = Vec::new();
        // TODO: Maybe, make the copies in parallel on each thread, rather than up front?
        for build_dir in build_dirs {
            threads.push(scope.spawn(|| {
                let build_dir = build_dir; // move it into this thread
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
                            &build_dir,
                            &output_mutex,
                            &Scenario::Mutant(mutant),
                            &[&package],
                            test_timeout,
                            &options,
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

fn test_timeout(
    baseline_outcome: &Option<ScenarioOutcome>,
    options: &Options,
    console: &Console,
) -> Duration {
    if let Some(timeout) = options.test_timeout {
        timeout
    } else if options.check_only {
        Duration::ZERO
    } else if options.baseline == BaselineStrategy::Skip {
        warn!("An explicit timeout is recommended when using --baseline=skip; using 300 seconds by default");
        Duration::from_secs(300)
    } else {
        let auto_timeout = max(
            options.minimum_test_timeout,
            baseline_outcome
                .as_ref()
                .expect("Baseline tests should have run")
                .total_phase_duration(Phase::Test)
                .mul_f32(5.0),
        );
        if options.show_times {
            // TODO: Just `info!` not a special method.
            console.autoset_timeout(auto_timeout);
        }
        auto_timeout
    }
}

/// Test various phases of one scenario in a build dir.
///
/// The [BuildDir] is passed as mutable because it's for the exclusive use of this function for the
/// duration of the test.
fn test_scenario(
    build_dir: &BuildDir,
    output_mutex: &Mutex<OutputDir>,
    scenario: &Scenario,
    test_packages: &[&Package],
    test_timeout: Duration,
    options: &Options,
    console: &Console,
) -> Result<ScenarioOutcome> {
    let mut log_file = output_mutex
        .lock()
        .expect("lock output_dir to create log")
        .create_log(scenario)?;
    log_file.message(&scenario.to_string());
    let applied = scenario
        .mutant()
        .map(|mutant| {
            // TODO: This is slightly inefficient as it computes the mutated source twice,
            // once for the diff and once to write it out.
            log_file.message(&format!("mutation diff:\n{}", mutant.diff()));
            mutant.apply(build_dir)
        })
        .transpose()?;
    console.scenario_started(scenario, log_file.path())?;

    let mut outcome = ScenarioOutcome::new(&log_file, scenario.clone());
    let phases: &[Phase] = if options.check_only {
        &[Phase::Check]
    } else {
        &[Phase::Build, Phase::Test]
    };
    for &phase in phases {
        console.scenario_phase_started(scenario, phase);
        let timeout = match phase {
            Phase::Test => test_timeout,
            _ => Duration::MAX,
        };
        let phase_result = run_cargo(
            build_dir,
            Some(test_packages),
            phase,
            timeout,
            &mut log_file,
            options,
            console,
        )?;
        let success = phase_result.is_success();
        outcome.add_phase_result(phase_result);
        console.scenario_phase_finished(scenario, phase);
        if (phase == Phase::Check && options.check_only) || !success {
            break;
        }
    }
    drop(applied);
    output_mutex
        .lock()
        .expect("lock output dir to add outcome")
        .add_scenario_outcome(&outcome)?;
    debug!(outcome = ?outcome.summary());
    console.scenario_finished(scenario, &outcome, options);

    Ok(outcome)
}

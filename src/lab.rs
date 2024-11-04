// Copyright 2021-2024 Martin Pool

//! Successively apply mutations to the source code and run cargo to check,
//! build, and test them.

use std::cmp::{max, min};
use std::panic::resume_unwind;
use std::sync::Mutex;
use std::thread;
use std::time::Instant;

use itertools::Itertools;
use tracing::{debug, debug_span, error, trace, warn};

use crate::*;
use crate::{
    cargo::run_cargo, options::TestPackages, outcome::LabOutcome, output::OutputDir,
    package::Package, timeouts::Timeouts,
};

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
    output_dir: OutputDir,
    options: Options,
    console: &Console,
) -> Result<LabOutcome> {
    let start_time = Instant::now();
    console.set_debug_log(output_dir.open_debug_log()?);
    let jobserver = options
        .jobserver
        .then(|| {
            let n_tasks = options.jobserver_tasks.unwrap_or_else(num_cpus::get);
            debug!(n_tasks, "starting jobserver");
            jobserver::Client::new(n_tasks)
        })
        .transpose()
        .context("Start jobserver")?;
    if options.shuffle {
        fastrand::shuffle(&mut mutants);
    }
    output_dir.write_mutants_list(&mutants)?;
    console.discovered_mutants(&mutants);
    if mutants.is_empty() {
        warn!("No mutants found under the active filters");
        return Ok(LabOutcome::default());
    }
    let mutant_packages = mutants.iter().map(|m| m.package()).unique().collect_vec(); // hold
    debug!(?mutant_packages);

    let output_mutex = Mutex::new(output_dir);
    let build_dir = match options.in_place {
        true => BuildDir::in_place(workspace_dir)?,
        false => BuildDir::copy_from(workspace_dir, options.gitignore, options.leak_dirs, console)?,
    };

    let timeouts = match options.baseline {
        BaselineStrategy::Run => {
            let outcome = test_scenario(
                &build_dir,
                &output_mutex,
                &jobserver,
                &Scenario::Baseline,
                Some(&mutant_packages),
                Timeouts::for_baseline(&options),
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
            Timeouts::from_baseline(&outcome, &options)
        }
        BaselineStrategy::Skip => Timeouts::without_baseline(&options),
    };
    debug!(?timeouts);

    let build_dir_0 = Mutex::new(Some(build_dir));
    // Create n threads, each dedicated to one build directory. Each of them tries to take a
    // scenario to test off the queue, and then exits when there are no more left.
    console.start_testing_mutants(mutants.len());
    let n_threads = max(1, min(options.jobs.unwrap_or(1), mutants.len()));
    let pending = Mutex::new(mutants.into_iter());
    thread::scope(|scope| -> crate::Result<()> {
        let mut threads = Vec::new();
        for _i_thread in 0..n_threads {
            threads.push(scope.spawn(|| -> crate::Result<()> {
                trace!(thread_id = ?thread::current().id(), "start thread");
                // First thread to start can use the initial build dir; others need to copy a new one
                let build_dir_0 = build_dir_0.lock().expect("lock build dir 0").take(); // separate for lock
                let build_dir = match build_dir_0 {
                    Some(d) => d,
                    None => {
                        debug!("copy build dir");
                        BuildDir::copy_from(
                            workspace_dir,
                            options.gitignore,
                            options.leak_dirs,
                            console,
                        )?
                    }
                };
                let _thread_span =
                    debug_span!("worker thread", build_dir = ?build_dir.path()).entered();
                loop {
                    // Extract the mutant in a separate statement so that we don't hold the
                    // lock while testing it.
                    let next = pending.lock().map(|mut s| s.next()); // separate for lock
                    match next {
                        Err(err) => {
                            // PoisonError is not Send so we can't pass it directly.
                            return Err(anyhow!("Failed to lock pending work queue: {}", err));
                        }
                        Ok(Some(mutant)) => {
                            let scenario = Scenario::Mutant(mutant.clone());
                            let _span =
                                debug_span!("mutant", name = mutant.name(false, false)).entered();
                            let package = mutant.package().clone(); // hold
                            let packages = [&package];
                            let test_packages: Option<&[&Package]> = match &options.test_packages {
                                TestPackages::Workspace => None,
                                TestPackages::Mutated => Some(&packages),
                                TestPackages::Named(_named) => {
                                    unimplemented!("get packages by name")
                                }
                            };
                            test_scenario(
                                &build_dir,
                                &output_mutex,
                                &jobserver,
                                &scenario,
                                test_packages,
                                timeouts,
                                &options,
                                console,
                            )?;
                        }
                        Ok(None) => {
                            return Ok(()); // no more work for this thread
                        }
                    }
                }
            }));
        }
        // The errors potentially returned from `join` are a special `std::thread::Result`
        // that does not implement error, indicating that the thread panicked.
        // Probably the most useful thing is to `resume_unwind` it.
        // Inside that, there's an actual Mutants error indicating a non-panic error.
        // Most likely, this would be "interrupted" but it might be some IO error
        // etc. In that case, print them all and return the first.
        let errors = threads
            .into_iter()
            .flat_map(|thread| match thread.join() {
                Err(panic) => resume_unwind(panic),
                Ok(Ok(())) => None,
                Ok(Err(err)) => {
                    // To avoid spam, as a special case, don't print "interrupted" errors for each thread,
                    // since that should have been printed by check_interrupted: but, do return them.
                    if err.to_string() != "interrupted" {
                        error!("Worker thread failed: {:?}", err);
                    }
                    Some(err)
                }
            })
            .collect_vec(); // print/process them all
        if let Some(first_err) = errors.into_iter().next() {
            Err(first_err)
        } else {
            Ok(())
        }
    })?;

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
#[allow(clippy::too_many_arguments)] // I agree it's a lot but I'm not sure wrapping in a struct would be better.
fn test_scenario(
    build_dir: &BuildDir,
    output_mutex: &Mutex<OutputDir>,
    jobserver: &Option<jobserver::Client>,
    scenario: &Scenario,
    test_packages: Option<&[&Package]>,
    timeouts: Timeouts,
    options: &Options,
    console: &Console,
) -> Result<ScenarioOutcome> {
    let mut scenario_output = output_mutex
        .lock()
        .expect("lock output_dir to start scenario")
        .start_scenario(scenario)?;
    let dir = build_dir.path();
    console.scenario_started(dir, scenario, scenario_output.open_log_read()?)?;

    let phases: &[Phase] = if options.check_only {
        &[Phase::Check]
    } else {
        &[Phase::Build, Phase::Test]
    };
    if let Some(mutant) = scenario.mutant() {
        let mutated_code = mutant.mutated_code();
        let diff = scenario.mutant().unwrap().diff(&mutated_code);
        scenario_output.write_diff(&diff)?;
        mutant.apply(build_dir, &mutated_code)?;
    }

    let mut outcome = ScenarioOutcome::new(&scenario_output, scenario.clone());
    for &phase in phases {
        console.scenario_phase_started(dir, phase);
        let timeout = match phase {
            Phase::Test => timeouts.test,
            Phase::Build | Phase::Check => timeouts.build,
        };
        match run_cargo(
            build_dir,
            jobserver,
            test_packages,
            phase,
            timeout,
            &mut scenario_output,
            options,
            console,
        ) {
            Ok(phase_result) => {
                let success = phase_result.is_success(); // so we can move it away
                outcome.add_phase_result(phase_result);
                console.scenario_phase_finished(dir, phase);
                if !success {
                    break;
                }
            }
            Err(err) => {
                // Some unexpected internal error that stops the program.
                if let Some(mutant) = scenario.mutant() {
                    mutant.revert(build_dir)?;
                    return Err(err);
                }
            }
        }
    }
    if let Some(mutant) = scenario.mutant() {
        mutant.revert(build_dir)?;
    }
    output_mutex
        .lock()
        .expect("lock output dir to add outcome")
        .add_scenario_outcome(&outcome)?;
    debug!(outcome = ?outcome.summary());
    console.scenario_finished(dir, scenario, &outcome, options);

    Ok(outcome)
}

// Copyright 2021-2024 Martin Pool

//! Successively apply mutations to the source code and run cargo to check,
//! build, and test them.

#![warn(clippy::pedantic)]

use std::cmp::{max, min};
use std::panic::resume_unwind;
use std::sync::Mutex;
use std::time::Instant;
use std::{thread, vec};

use itertools::Itertools;
use tracing::{debug, debug_span, error, trace, warn};

use crate::{
    cargo::run_cargo, options::TestPackages, outcome::LabOutcome, output::OutputDir,
    timeouts::Timeouts, workspace::Workspace, BaselineStrategy, BuildDir, Console, Context, Mutant,
    Options, Phase, Result, Scenario, ScenarioOutcome,
};

/// Run all possible mutation experiments.
///
/// This is called after all filtering is complete, so all the mutants here will be tested
/// or checked.
///
/// Before testing the mutants, the lab checks that the source tree passes its tests with no
/// mutations applied.
#[allow(clippy::too_many_lines)] // just for now
pub fn test_mutants(
    mut mutants: Vec<Mutant>,
    workspace: &Workspace,
    output_dir: OutputDir,
    options: &Options,
    console: &Console,
) -> Result<LabOutcome> {
    let start_time = Instant::now();
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

    let output_mutex = Mutex::new(output_dir);
    let baseline_build_dir = BuildDir::for_baseline(workspace, options, console)?;
    let jobserver = options
        .jobserver
        .then(|| {
            let n_tasks = options.jobserver_tasks.unwrap_or_else(num_cpus::get);
            debug!(n_tasks, "starting jobserver");
            jobserver::Client::new(n_tasks)
        })
        .transpose()
        .context("Start jobserver")?;
    let lab = Lab {
        workspace,
        output_mutex,
        jobserver,
        options,
        console,
    };
    let timeouts = match options.baseline {
        BaselineStrategy::Run => {
            let outcome = lab.run_baseline(&baseline_build_dir, &mutants)?;
            if outcome.success() {
                Timeouts::from_baseline(&outcome, options)
            } else {
                error!(
                    "cargo {phase} failed in an unmutated tree, so no mutants were tested",
                    phase = outcome.last_phase(),
                );
                return Ok(lab
                    .output_mutex
                    .into_inner()
                    .expect("lock output_dir")
                    .take_lab_outcome());
            }
        }
        BaselineStrategy::Skip => Timeouts::without_baseline(options),
    };
    debug!(?timeouts);

    let build_dir_0 = Mutex::new(Some(baseline_build_dir));
    // Create n threads, each dedicated to one build directory. Each of them tries to take a
    // scenario to test off the queue, and then exits when there are no more left.
    console.start_testing_mutants(mutants.len());
    let n_threads = max(1, min(options.jobs.unwrap_or(1), mutants.len()));
    let work_queue = &Mutex::new(mutants.into_iter());
    thread::scope(|scope| -> crate::Result<()> {
        let mut threads = Vec::new();
        for _i_thread in 0..n_threads {
            threads.push(scope.spawn(|| -> crate::Result<()> {
                trace!(thread_id = ?thread::current().id(), "start thread");
                // First thread to start can use the baseline's build dir;
                // others need to copy a new one
                let build_dir_0 = build_dir_0.lock().expect("lock build dir 0").take(); // separate for lock
                let build_dir = &if let Some(d) = build_dir_0 {
                    d
                } else {
                    BuildDir::copy_from(workspace.root(), options, console)?
                };
                lab.run_queue(build_dir, timeouts, work_queue)
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
            .filter_map(|thread| match thread.join() {
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

    let output_dir = lab
        .output_mutex
        .into_inner()
        .expect("final unlock mutants queue");
    console.lab_finished(&output_dir.lab_outcome, start_time, options);
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

/// Common context across all scenarios, threads, and build dirs.
struct Lab<'a> {
    #[allow(unused)] // needed later to find packages
    workspace: &'a Workspace,
    output_mutex: Mutex<OutputDir>,
    jobserver: Option<jobserver::Client>,
    options: &'a Options,
    console: &'a Console,
}

impl Lab<'_> {
    /// Run the baseline scenario, which is the same as running `cargo test` on the unmutated
    /// tree.
    ///
    /// If it fails, return None, indicating that no further testing should be done.
    ///
    /// If it succeeds, return the timeouts to be used for the other scenarios.
    fn run_baseline(
        &self,
        baseline_build_dir: &BuildDir,
        mutants: &[Mutant],
    ) -> Result<ScenarioOutcome> {
        let all_mutated_packages = mutants
            .iter()
            .map(|m| m.source_file.package_name.as_str())
            .unique()
            .collect_vec();
        Worker {
            build_dir: baseline_build_dir,
            output_mutex: &self.output_mutex,
            jobserver: &self.jobserver,
            timeouts: Timeouts::for_baseline(self.options),
            options: self.options,
            console: self.console,
        }
        .run_one_scenario(&Scenario::Baseline, Some(&all_mutated_packages))
    }

    /// Run until the input queue is empty.
    ///
    /// The queue, inside a mutex, can be consumed by multiple threads.
    fn run_queue(
        &self,
        build_dir: &BuildDir,
        timeouts: Timeouts,
        work_queue: &Mutex<vec::IntoIter<Mutant>>,
    ) -> Result<()> {
        Worker {
            build_dir,
            output_mutex: &self.output_mutex,
            jobserver: &self.jobserver,
            timeouts,
            options: self.options,
            console: self.console,
        }
        .run_queue(work_queue)
    }
}

/// A worker owns one build directory and runs a single thread of testing.
///
/// It consumes jobs from an input queue and runs them until the queue is empty,
/// appending output to the output directory.
struct Worker<'a> {
    build_dir: &'a BuildDir,
    output_mutex: &'a Mutex<OutputDir>,
    jobserver: &'a Option<jobserver::Client>,
    timeouts: Timeouts,
    options: &'a Options,
    console: &'a Console,
}

impl Worker<'_> {
    /// Run until the input queue is empty.
    fn run_queue(mut self, work_queue: &Mutex<vec::IntoIter<Mutant>>) -> Result<()> {
        let _thread_span =
            debug_span!("worker thread", build_dir = ?self.build_dir.path()).entered();
        loop {
            // Extract the mutant in a separate statement so that we don't hold the
            // lock while testing it.
            let next_mutant = work_queue.lock().expect("Lock pending work queue").next(); // separate for lock
            if let Some(mutant) = next_mutant {
                let _span = debug_span!("mutant", name = mutant.name(false, false)).entered();
                let package_name = mutant.source_file.package_name.clone(); // hold
                let scenario = Scenario::Mutant(mutant);
                let local_packages: &[&str] = &[&package_name]; // hold
                let test_packages: Option<&[&str]> = match &self.options.test_packages {
                    TestPackages::Workspace => None,
                    TestPackages::Mutated => Some(local_packages),
                    TestPackages::Named(_named) => {
                        unimplemented!("get packages by name")
                    }
                };
                self.run_one_scenario(&scenario, test_packages)?;
            } else {
                return Ok(());
            }
        }
    }

    fn run_one_scenario(
        &mut self,
        scenario: &Scenario,
        test_packages: Option<&[&str]>,
    ) -> Result<ScenarioOutcome> {
        let mut scenario_output = self
            .output_mutex
            .lock()
            .expect("lock output_dir to start scenario")
            .start_scenario(scenario)?;
        let dir = self.build_dir.path();
        self.console
            .scenario_started(dir, scenario, scenario_output.open_log_read()?)?;

        if let Some(mutant) = scenario.mutant() {
            let mutated_code = mutant.mutated_code();
            let diff = scenario.mutant().unwrap().diff(&mutated_code);
            scenario_output.write_diff(&diff)?;
            mutant.apply(self.build_dir, &mutated_code)?;
        }

        let mut outcome = ScenarioOutcome::new(&scenario_output, scenario.clone());
        for &phase in self.options.phases() {
            self.console.scenario_phase_started(dir, phase);
            let timeout = match phase {
                Phase::Test => self.timeouts.test,
                Phase::Build | Phase::Check => self.timeouts.build,
            };
            match run_cargo(
                self.build_dir,
                self.jobserver,
                test_packages,
                phase,
                timeout,
                &mut scenario_output,
                self.options,
                self.console,
            ) {
                Ok(phase_result) => {
                    let success = phase_result.is_success(); // so we can move it away
                    outcome.add_phase_result(phase_result);
                    self.console.scenario_phase_finished(dir, phase);
                    if !success {
                        break;
                    }
                }
                Err(err) => {
                    error!(?err, ?phase, "scenario execution internal error");
                    // Some unexpected internal error that stops the program.
                    if let Some(mutant) = scenario.mutant() {
                        mutant.revert(self.build_dir)?;
                    }
                    return Err(err);
                }
            }
        }
        if let Some(mutant) = scenario.mutant() {
            mutant.revert(self.build_dir)?;
        }
        self.output_mutex
            .lock()
            .expect("lock output dir to add outcome")
            .add_scenario_outcome(&outcome)?;
        debug!(outcome = ?outcome.summary());
        self.console
            .scenario_finished(dir, scenario, &outcome, self.options);

        Ok(outcome)
    }
}

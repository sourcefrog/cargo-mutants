// Copyright 2022-2024 Martin Pool

//! The outcome of running a single mutation scenario, or a whole lab.

use std::fmt;
use std::fs::read_to_string;
use std::time::{Duration, Instant};

use anyhow::Context;
use camino::Utf8PathBuf;
use jiff::Timestamp;
use output::ScenarioOutput;
use serde::Serialize;
use serde::Serializer;
use serde::ser::SerializeStruct;
use tracing::warn;

use crate::console::{format_duration, plural};
use crate::exit_code::ExitCode;
#[cfg(unix)]
use crate::process::signal_name;
use crate::process::{Exit, ProcessReport};
use crate::{Options, Result, Scenario, output};

/// What phase of running a scenario.
///
/// Every scenario proceed through up to three phases in order. They are:
///
/// 1. `cargo check` -- is the tree basically buildable? This is skipped
///    during normal testing, but used with `--check`, in which case the
///    other phases are skipped.
/// 2. `cargo build` -- actually build it.
/// 3. `cargo tests` -- do the tests pass?
///
/// Some scenarios such as freshening the tree don't run the tests.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Serialize)]
pub enum Phase {
    Check,
    Build,
    Test,
}

impl Phase {
    pub fn name(self) -> &'static str {
        match self {
            Phase::Check => "check",
            Phase::Build => "build",
            Phase::Test => "test",
        }
    }
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.name())
    }
}

/// The outcome from a whole lab run containing multiple mutants.
#[derive(Debug, Serialize)]
#[allow(clippy::module_name_repetitions)]
pub struct LabOutcome {
    /// All the scenario outcomes, including baseline builds.
    pub outcomes: Vec<ScenarioOutcome>,
    pub total_mutants: usize,
    pub missed: usize,
    pub caught: usize,
    pub timeout: usize,
    pub unviable: usize,
    pub success: usize,
    /// How many scenarios had something OOM-killed in their memory cgroup.
    ///
    /// Not a category of its own -- an OOM-killed mutant is also a caught one -- so it is
    /// reported alongside the counts rather than in them, and kept out of the JSON, where
    /// each phase already carries its own report.
    #[serde(skip)]
    pub oom_killed: usize,
    pub start_time: Timestamp,
    pub end_time: Option<Timestamp>,
    pub cargo_mutants_version: String,
}

impl LabOutcome {
    pub fn new(start_time: Timestamp) -> LabOutcome {
        LabOutcome {
            outcomes: Vec::new(),
            total_mutants: 0,
            missed: 0,
            caught: 0,
            timeout: 0,
            unviable: 0,
            success: 0,
            oom_killed: 0,
            start_time,
            end_time: None,
            cargo_mutants_version: crate::VERSION.to_string(),
        }
    }

    /// Record the event of one test.
    pub fn add(&mut self, outcome: ScenarioOutcome) {
        // Counted for the baseline too: if the unmutated tree can't fit in the limit,
        // that's the most important thing to say about the run.
        if outcome.was_oom_killed() {
            self.oom_killed += 1;
        }
        if outcome.scenario.is_mutant() {
            self.total_mutants += 1;
            match outcome.summary() {
                SummaryOutcome::CaughtMutant => self.caught += 1,
                SummaryOutcome::MissedMutant => self.missed += 1,
                SummaryOutcome::Timeout => self.timeout += 1,
                SummaryOutcome::Unviable => self.unviable += 1,
                SummaryOutcome::Success => self.success += 1,
                SummaryOutcome::Failure => {
                    // We don't expect to see failures that don't fit into the other categories.
                    warn!("Unclassified failure for mutant {:?}", outcome.scenario);
                }
            }
        }
        self.outcomes.push(outcome);
    }

    /// Return the overall program exit code reflecting this outcome.
    pub fn exit_code(&self) -> ExitCode {
        // TODO: Maybe move this into an error returned from experiment()?
        if self
            .outcomes
            .iter()
            .any(|o| !o.scenario.is_mutant() && !o.success())
        {
            ExitCode::BaselineFailed
        } else if self.timeout > 0 {
            ExitCode::Timeout
        } else if self.missed > 0 {
            ExitCode::FoundProblems
        } else {
            ExitCode::Success
        }
    }

    /// Return an overall summary, to show at the end of the program.
    pub fn summary_string(&self, start_time: Instant, options: &Options) -> String {
        let mut s = Vec::new();
        s.push(format!("{} tested", plural(self.total_mutants, "mutant")));
        if options.show_times {
            s.push(format!(" in {}", format_duration(start_time.elapsed())));
        }
        s.push(": ".into());
        let mut by_outcome: Vec<String> = Vec::new();
        if self.missed != 0 {
            by_outcome.push(format!("{} missed", self.missed));
        }
        if self.caught != 0 {
            by_outcome.push(format!("{} caught", self.caught));
        }
        if self.unviable != 0 {
            by_outcome.push(format!("{} unviable", self.unviable));
        }
        if self.timeout != 0 {
            by_outcome.push(format!("{} timeouts", self.timeout));
        }
        if self.success != 0 {
            by_outcome.push(format!("{} succeeded", self.success));
        }
        s.push(by_outcome.join(", "));
        if self.oom_killed > 0 {
            s.push(format!(
                " ({} stopped by the --max-memory limit)",
                self.oom_killed
            ));
        }
        s.join("")
    }
}

/// The result of running one mutation scenario.
#[derive(Debug, Clone, Eq, PartialEq)]
#[allow(clippy::module_name_repetitions)]
pub struct ScenarioOutcome {
    /// A file holding the text output from running this test.
    // TODO: Maybe this should be a log object?
    output_dir: Utf8PathBuf,
    log_path: Utf8PathBuf,
    /// The path relative to `mutants.out` for a file showing the diff between the unmutated
    /// and mutated source. Only present for mutant scenarios.
    diff_path: Option<Utf8PathBuf>,
    /// What kind of scenario was being built?
    pub scenario: Scenario,
    /// For each phase, the duration and the cargo result.
    phase_results: Vec<PhaseResult>,
}

impl Serialize for ScenarioOutcome {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // custom serialize to omit inessential info and to inline a summary.
        let mut ss = serializer.serialize_struct("Outcome", 5)?;
        ss.serialize_field("scenario", &self.scenario)?;
        ss.serialize_field("summary", &self.summary())?;
        ss.serialize_field("log_path", &self.log_path)?;
        ss.serialize_field("diff_path", &self.diff_path)?;
        ss.serialize_field("phase_results", &self.phase_results)?;
        ss.end()
    }
}

impl ScenarioOutcome {
    pub fn new(scenario_output: &ScenarioOutput, scenario: Scenario) -> ScenarioOutcome {
        ScenarioOutcome {
            output_dir: scenario_output.output_dir.clone(),
            log_path: scenario_output.log_path().to_owned(),
            diff_path: scenario_output.diff_path.clone(),
            scenario,
            phase_results: Vec::new(),
        }
    }

    pub fn add_phase_result(&mut self, phase_result: PhaseResult) {
        self.phase_results.push(phase_result);
    }

    pub fn get_log_content(&self) -> Result<String> {
        read_to_string(self.output_dir.join(&self.log_path)).context("read log file")
    }

    pub fn last_phase(&self) -> Phase {
        self.phase_results.last().unwrap().phase
    }

    pub fn last_phase_result(&self) -> Exit {
        self.phase_results.last().unwrap().process_status
    }

    /// Return the results of all phases.
    pub fn phase_results(&self) -> &[PhaseResult] {
        &self.phase_results
    }

    /// Return the result of the given phase, if it was run.
    pub fn phase_result(&self, phase: Phase) -> Option<&PhaseResult> {
        self.phase_results.iter().find(|pr| pr.phase == phase)
    }

    /// True if this status indicates the user definitely needs to see the logs, because a task
    /// failed that should not have failed.
    pub fn should_show_logs(&self) -> bool {
        !self.scenario.is_mutant() && !self.success()
    }

    pub fn success(&self) -> bool {
        self.last_phase_result().is_success()
    }

    pub fn has_timeout(&self) -> bool {
        self.phase_results
            .iter()
            .any(|pr| pr.process_status.is_timeout())
    }

    pub fn check_or_build_failed(&self) -> bool {
        self.phase_results
            .iter()
            .any(|pr| pr.phase != Phase::Test && pr.process_status.is_failure())
    }

    /// Say, for each phase that has something unusual to report, how its process tree
    /// ended: killed by a signal, stopped by the memory limit, or leaving strays behind.
    ///
    /// This has no bearing on how the mutant is classified. It exists so that a mutant
    /// caught because the kernel OOM-killed its tests can be told apart from one caught
    /// by a failing assertion.
    pub fn death_reasons(&self) -> Vec<String> {
        self.phase_results
            .iter()
            .flat_map(PhaseResult::death_reasons)
            .collect()
    }

    /// True if the kernel OOM-killed anything in this scenario's memory cgroup.
    pub fn was_oom_killed(&self) -> bool {
        self.phase_results
            .iter()
            .any(|pr| pr.report.oom_kills.is_some_and(|n| n > 0))
    }

    /// True if this outcome is a caught mutant: it's a mutant and the tests failed.
    pub fn mutant_caught(&self) -> bool {
        self.scenario.is_mutant()
            && self.last_phase() == Phase::Test
            && self.last_phase_result().is_failure()
    }

    /// True if this outcome is a missed mutant: it's a mutant and the tests succeeded.
    pub fn mutant_missed(&self) -> bool {
        self.scenario.is_mutant()
            && self.last_phase() == Phase::Test
            && self.last_phase_result().is_success()
    }

    pub fn summary(&self) -> SummaryOutcome {
        // Caution: this function is called when rendering progress
        // and so should not log; see https://github.com/sourcefrog/nutmeg/issues/16.
        match self.scenario {
            Scenario::Baseline => {
                if self.has_timeout() {
                    SummaryOutcome::Timeout
                } else if self.success() {
                    SummaryOutcome::Success
                } else {
                    SummaryOutcome::Failure
                }
            }
            Scenario::Mutant(_) => {
                if self.check_or_build_failed() {
                    SummaryOutcome::Unviable
                } else if self.has_timeout() {
                    SummaryOutcome::Timeout
                } else if self.mutant_caught() {
                    SummaryOutcome::CaughtMutant
                } else if self.mutant_missed() {
                    SummaryOutcome::MissedMutant
                } else if self.success() {
                    SummaryOutcome::Success
                } else {
                    // Some unattributed failure; should be rare or impossible?
                    SummaryOutcome::Failure
                }
            }
        }
    }
}

/// The result of running one phase of a mutation scenario, i.e. a single cargo check/build/test command.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PhaseResult {
    /// What phase was this?
    pub phase: Phase,
    /// How long did it take?
    pub duration: Duration,
    /// Did it succeed?
    pub process_status: Exit,
    /// What command was run, as an argv list.
    pub argv: Vec<String>,
    /// What became of the process tree, beyond the exit status.
    pub report: ProcessReport,
}

impl PhaseResult {
    pub fn is_success(&self) -> bool {
        self.process_status.is_success()
    }

    fn death_reasons(&self) -> Vec<String> {
        let phase = self.phase.name();
        let mut reasons = Vec::new();
        #[cfg(unix)]
        if let Exit::Signalled(signal) = self.process_status {
            reasons.push(format!("{phase} killed by {}", signal_name(signal)));
        }
        if let Some(oom_kills) = self.report.oom_kills.filter(|n| *n > 0) {
            reasons.push(format!(
                "{phase} OOM-killed by the kernel ({oom_kills} process{es}) for exceeding the memory limit",
                es = if oom_kills == 1 { "" } else { "es" }
            ));
        }
        if let Some(sweep) = self.report.sweep.describe() {
            reasons.push(format!("{phase} {sweep}"));
        }
        reasons
    }
}

impl Serialize for PhaseResult {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ss = serializer.serialize_struct("PhaseResult", 5)?;
        ss.serialize_field("phase", &self.phase)?;
        ss.serialize_field("duration", &self.duration.as_secs_f64())?;
        ss.serialize_field("process_status", &self.process_status)?;
        ss.serialize_field("argv", &self.argv)?;
        ss.serialize_field("report", &self.report)?;
        ss.end()
    }
}

/// Overall summary outcome for one mutant.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Hash)]
#[allow(clippy::module_name_repetitions)]
pub enum SummaryOutcome {
    Success,
    CaughtMutant,
    MissedMutant,
    Unviable,
    Failure,
    Timeout,
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use crate::process::{Exit, ProcessReport, Sweep};

    use super::{Phase, PhaseResult, Scenario, ScenarioOutcome};

    fn phase_result(phase: Phase, process_status: Exit, report: ProcessReport) -> PhaseResult {
        PhaseResult {
            phase,
            duration: Duration::from_secs(1),
            process_status,
            argv: vec!["cargo".into(), "test".into()],
            report,
        }
    }

    fn outcome_of(phase_results: Vec<PhaseResult>) -> ScenarioOutcome {
        ScenarioOutcome {
            output_dir: "output".into(),
            log_path: "log".into(),
            diff_path: None,
            scenario: Scenario::Baseline,
            phase_results,
        }
    }

    #[test]
    fn no_death_reasons_for_an_ordinary_test_failure() {
        let outcome = outcome_of(vec![phase_result(
            Phase::Test,
            Exit::Failure(101),
            ProcessReport::default(),
        )]);
        assert_eq!(outcome.death_reasons(), Vec::<String>::new());
    }

    #[cfg(unix)]
    #[test]
    fn death_reasons_name_the_signal_that_killed_the_phase() {
        let outcome = outcome_of(vec![phase_result(
            Phase::Test,
            Exit::Signalled(9),
            ProcessReport::default(),
        )]);
        assert_eq!(outcome.death_reasons(), ["test killed by SIGKILL"]);
    }

    #[test]
    fn death_reasons_name_an_oom_kill() {
        let outcome = outcome_of(vec![
            phase_result(Phase::Build, Exit::Success, ProcessReport::default()),
            phase_result(
                Phase::Test,
                Exit::Failure(101),
                ProcessReport {
                    oom_kills: Some(1),
                    ..ProcessReport::default()
                },
            ),
        ]);
        assert_eq!(
            outcome.death_reasons(),
            ["test OOM-killed by the kernel (1 process) for exceeding the memory limit"]
        );
    }

    /// A cgroup that was watched but never hit its limit has nothing to say.
    #[test]
    fn no_death_reason_for_zero_oom_kills() {
        let outcome = outcome_of(vec![phase_result(
            Phase::Test,
            Exit::Success,
            ProcessReport {
                oom_kills: Some(0),
                ..ProcessReport::default()
            },
        )]);
        assert_eq!(outcome.death_reasons(), Vec::<String>::new());
    }

    #[test]
    fn death_reasons_name_processes_left_behind_by_the_tests() {
        let outcome = outcome_of(vec![phase_result(
            Phase::Test,
            Exit::Success,
            ProcessReport {
                sweep: Sweep {
                    pids: Some(vec![101, 102]),
                    strays: true,
                    killed: true,
                },
                oom_kills: None,
            },
        )]);
        assert_eq!(
            outcome.death_reasons(),
            ["test left 2 stray processes behind (SIGKILLed: 101, 102)"]
        );
    }

    #[test]
    fn find_phase_result() {
        let outcome = ScenarioOutcome {
            output_dir: "output".into(),
            log_path: "log".into(),
            diff_path: Some("mutant.diff".into()),
            scenario: Scenario::Baseline,
            phase_results: vec![
                PhaseResult {
                    phase: Phase::Build,
                    duration: Duration::from_secs(2),
                    process_status: Exit::Success,
                    argv: vec!["cargo".into(), "build".into()],
                    report: ProcessReport::default(),
                },
                PhaseResult {
                    phase: Phase::Test,
                    duration: Duration::from_secs(3),
                    process_status: Exit::Success,
                    argv: vec!["cargo".into(), "test".into()],
                    report: ProcessReport::default(),
                },
            ],
        };
        assert_eq!(
            outcome.phase_result(Phase::Build),
            Some(&PhaseResult {
                phase: Phase::Build,
                duration: Duration::from_secs(2),
                process_status: Exit::Success,
                argv: vec!["cargo".into(), "build".into()],
                report: ProcessReport::default(),
            })
        );
        assert_eq!(
            outcome
                .phase_result(Phase::Build)
                .unwrap()
                .duration
                .as_secs(),
            2
        );
        assert_eq!(
            outcome
                .phase_result(Phase::Test)
                .unwrap()
                .duration
                .as_secs(),
            3
        );
        assert_eq!(outcome.phase_result(Phase::Check), None);
    }
}

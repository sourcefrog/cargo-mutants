// Copyright 2022-2023 Martin Pool

//! The outcome of running a single mutation scenario, or a whole lab.

use std::fmt;
use std::fs;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use serde::ser::SerializeStruct;
use serde::Deserialize;
use serde::Serialize;
use serde::Serializer;

use crate::console::{duration_minutes_seconds, plural};
use crate::exit_code;
use crate::log_file::LogFile;
use crate::process::ProcessStatus;
use crate::*;

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
    pub fn name(&self) -> &'static str {
        match self {
            Phase::Check => "check",
            Phase::Build => "build",
            Phase::Test => "test",
        }
    }
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The outcome from a whole lab run containing multiple mutants.
#[derive(Debug, Default, Serialize)]
pub struct LabOutcome {
    /// All the scenario outcomes, including baseline builds.
    pub outcomes: Vec<ScenarioOutcome>,
    pub total_mutants: usize,
    pub missed: usize,
    pub caught: usize,
    pub timeout: usize,
    pub unviable: usize,
    pub success: usize,
    pub failure: usize,
}

impl LabOutcome {
    pub fn new() -> LabOutcome {
        LabOutcome::default()
    }

    /// Record the event of one test.
    pub fn add(&mut self, outcome: ScenarioOutcome) {
        if outcome.scenario.is_mutant() {
            self.total_mutants += 1;
            match outcome.summary() {
                SummaryOutcome::CaughtMutant => self.caught += 1,
                SummaryOutcome::MissedMutant => self.missed += 1,
                SummaryOutcome::Timeout => self.timeout += 1,
                SummaryOutcome::Unviable => self.unviable += 1,
                SummaryOutcome::Success => self.success += 1,
                SummaryOutcome::Failure => self.failure += 1,
            }
        }
        self.outcomes.push(outcome);
    }

    /// Return the overall program exit code reflecting this outcome.
    pub fn exit_code(&self) -> i32 {
        // TODO: Maybe move this into an error returned from experiment()?
        if self
            .outcomes
            .iter()
            .any(|o| !o.scenario.is_mutant() && !o.success())
        {
            exit_code::CLEAN_TESTS_FAILED
        } else if self.timeout > 0 {
            exit_code::TIMEOUT
        } else if self.missed > 0 {
            exit_code::FOUND_PROBLEMS
        } else {
            exit_code::SUCCESS
        }
    }

    /// Return an overall summary, to show at the end of the program.
    pub fn summary_string(&self, start_time: Instant, options: &Options) -> String {
        let mut s = format!("{} tested", plural(self.total_mutants, "mutant"),);
        if options.show_times {
            s.push_str(" in ");
            s.push_str(&duration_minutes_seconds(start_time.elapsed()));
        }
        s.push_str(": ");
        let mut parts: Vec<String> = Vec::new();
        if self.missed > 0 {
            parts.push(format!("{} missed", self.missed));
        }
        if self.caught > 0 {
            parts.push(format!("{} caught", self.caught));
        }
        if self.unviable > 0 {
            parts.push(format!("{} unviable", self.unviable));
        }
        if self.timeout > 0 {
            parts.push(format!("{} timeouts", self.timeout));
        }
        if self.success > 0 {
            parts.push(format!("{} succeeded", self.success));
        }
        if self.failure > 0 {
            parts.push(format!("{} failed", self.failure));
        }
        s.push_str(&parts.join(", "));
        s
    }
}

/// The result of running one mutation scenario.
#[derive(Debug, Clone, Eq, PartialEq)]
#[must_use]
pub struct ScenarioOutcome {
    /// A file holding the text output from running this test.
    // TODO: Maybe this should be a log object?
    log_path: Utf8PathBuf,
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
        // custom serialize to omit inessential info
        let mut ss = serializer.serialize_struct("Outcome", 4)?;
        ss.serialize_field("scenario", &self.scenario)?;
        ss.serialize_field("log_path", &self.log_path)?;
        ss.serialize_field("summary", &self.summary())?;
        ss.serialize_field("phase_results", &self.phase_results)?;
        ss.end()
    }
}

impl ScenarioOutcome {
    pub fn new(log_file: &LogFile, scenario: Scenario) -> ScenarioOutcome {
        ScenarioOutcome {
            log_path: log_file.path().to_owned(),
            scenario,
            phase_results: Vec::new(),
        }
    }

    pub fn add_phase_result(&mut self, phase_result: PhaseResult) {
        self.phase_results.push(phase_result);
    }

    pub fn get_log_content(&self) -> Result<String> {
        fs::read_to_string(&self.log_path).context("read log file")
    }

    pub fn last_phase(&self) -> Phase {
        self.phase_results.last().unwrap().phase
    }

    pub fn last_phase_result(&self) -> ProcessStatus {
        self.phase_results.last().unwrap().process_status
    }

    pub fn phase_results(&self) -> &[PhaseResult] {
        &self.phase_results
    }

    /// True if this status indicates the user definitely needs to see the logs, because a task
    /// failed that should not have failed.
    pub fn should_show_logs(&self) -> bool {
        !self.scenario.is_mutant() && !self.success()
    }

    pub fn success(&self) -> bool {
        self.last_phase_result().success()
    }

    pub fn has_timeout(&self) -> bool {
        self.phase_results
            .iter()
            .any(|pr| pr.process_status.timeout())
    }

    pub fn check_or_build_failed(&self) -> bool {
        self.phase_results
            .iter()
            .any(|pr| pr.phase != Phase::Test && pr.process_status == ProcessStatus::Failure)
    }

    /// True if this outcome is a caught mutant: it's a mutant and the tests failed.
    pub fn mutant_caught(&self) -> bool {
        self.scenario.is_mutant()
            && self.last_phase() == Phase::Test
            && self.last_phase_result() == ProcessStatus::Failure
    }

    /// True if this outcome is a missed mutant: it's a mutant and the tests succeeded.
    pub fn mutant_missed(&self) -> bool {
        self.scenario.is_mutant()
            && self.last_phase() == Phase::Test
            && self.last_phase_result().success()
    }

    pub fn summary(&self) -> SummaryOutcome {
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
    pub process_status: ProcessStatus,
    /// What command was run, as an argv list.
    pub argv: Vec<String>,
}

impl PhaseResult {
    pub fn is_success(&self) -> bool {
        self.process_status.success()
    }
}

impl Serialize for PhaseResult {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ss = serializer.serialize_struct("PhaseResult", 4)?;
        ss.serialize_field("phase", &self.phase)?;
        ss.serialize_field("duration", &self.duration.as_secs_f64())?;
        ss.serialize_field("process_status", &self.process_status)?;
        ss.serialize_field("argv", &self.argv)?;
        ss.end()
    }
}

/// Overall summary outcome for one mutant.
#[derive(Debug, Deserialize, Clone, Eq, PartialEq, Serialize, Hash)]
pub enum SummaryOutcome {
    Success,
    CaughtMutant,
    MissedMutant,
    Unviable,
    Failure,
    Timeout,
}

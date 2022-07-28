// Copyright 2022 Martin Pool

//! The outcome of running a single mutation scenario, or a whole lab.

use std::fmt;
use std::fs;
use std::time::Duration;

use anyhow::Context;
use serde::ser::SerializeStruct;
use serde::Serialize;
use serde::Serializer;

use crate::exit_code;
use crate::log_file::LogFile;
use crate::*;

/// What phase of running a scenario.
///
/// Every scenario proceed through up to three phases in order. They are:
///
/// 1. `cargo check` -- is the tree basically buildable; this should detect many
///    unviable mutants early.
/// 2. `cargo build` -- actually build it.
/// 3. `cargo tests` -- do the tests pass?
///
/// Some scenarios such as freshening the tree don't run the tests. Tests might
/// also be skipped by `--check`.
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

    pub const ALL: &'static [Phase] = &[Phase::Check, Phase::Build, Phase::Test];
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
    outcomes: Vec<Outcome>,
    total_mutants: usize,
    missed: usize,
    caught: usize,
    timeout: usize,
    unviable: usize,
    success: usize,
    failure: usize,
}

impl LabOutcome {
    /// Record the event of one test.
    pub fn add(&mut self, outcome: &Outcome) {
        self.outcomes.push(outcome.clone());
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
    pub fn summary_string(&self) -> String {
        let mut s = format!("{} mutants tested: ", self.total_mutants,);
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
            parts.push(format!("{} builds succeeded", self.success));
        }
        if self.failure > 0 {
            parts.push(format!("{} builds failed", self.failure));
        }
        s.push_str(&parts.join(", "));
        s
    }
}

/// The result of running one mutation scenario.
#[derive(Debug, Clone, Eq, PartialEq)]
#[must_use]
pub struct Outcome {
    /// A file holding the text output from running this test.
    // TODO: Maybe this should be a log object?
    log_path: Utf8PathBuf,
    /// What kind of scenario was being built?
    pub scenario: Scenario,
    /// For each phase, the duration and the cargo result.
    phase_results: Vec<PhaseResult>,
}

impl Serialize for Outcome {
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

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Hash)]
pub enum SummaryOutcome {
    Success,
    CaughtMutant,
    MissedMutant,
    Unviable,
    Failure,
    Timeout,
}

impl Outcome {
    pub fn new(log_file: &LogFile, scenario: Scenario) -> Outcome {
        Outcome {
            log_path: log_file.path().to_owned(),
            scenario,
            phase_results: Vec::new(),
        }
    }

    pub fn add_phase_result(
        &mut self,
        phase: Phase,
        duration: Duration,
        cargo_result: CargoResult,
    ) {
        self.phase_results.push(PhaseResult {
            phase,
            duration,
            cargo_result,
        });
    }

    pub fn get_log_content(&self) -> Result<String> {
        fs::read_to_string(&self.log_path).context("read log file")
    }

    pub fn last_phase(&self) -> Phase {
        self.phase_results.last().unwrap().phase
    }

    pub fn last_phase_result(&self) -> CargoResult {
        self.phase_results.last().unwrap().cargo_result
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
            .any(|pr| pr.cargo_result == CargoResult::Timeout)
    }

    pub fn check_or_build_failed(&self) -> bool {
        self.phase_results
            .iter()
            .any(|pr| pr.phase != Phase::Test && pr.cargo_result == CargoResult::Failure)
    }

    /// True if this outcome is a caught mutant: it's a mutant and the tests failed.
    pub fn mutant_caught(&self) -> bool {
        self.scenario.is_mutant()
            && self.last_phase() == Phase::Test
            && self.last_phase_result() == CargoResult::Failure
    }

    /// True if this outcome is a missed mutant: it's a mutant and the tests succeeded.
    pub fn mutant_missed(&self) -> bool {
        self.scenario.is_mutant()
            && self.last_phase() == Phase::Test
            && self.last_phase_result().success()
    }

    /// Duration of the test phase, if tests were run.
    pub fn test_duration(&self) -> Option<Duration> {
        if let Some(phase_result) = self.phase_results.last() {
            if phase_result.phase == Phase::Test {
                return Some(phase_result.duration);
            }
        }
        None
    }

    pub fn summary(&self) -> SummaryOutcome {
        match self.scenario {
            Scenario::SourceTree | Scenario::Baseline => {
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

#[derive(Debug, Clone, Eq, PartialEq)]
struct PhaseResult {
    phase: Phase,
    duration: Duration,
    cargo_result: CargoResult,
}

impl Serialize for PhaseResult {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // custom serialize to omit inessential info
        let mut ss = serializer.serialize_struct("PhaseResult", 3)?;
        ss.serialize_field("phase", &self.phase)?;
        ss.serialize_field("duration", &self.duration.as_secs_f64())?;
        ss.serialize_field("cargo_result", &self.cargo_result)?;
        ss.end()
    }
}

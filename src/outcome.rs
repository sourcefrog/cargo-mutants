// Copyright 2022 Martin Pool

//! The outcome of running a command.

use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use serde::Serialize;

use crate::exit_code;
use crate::log_file::LogFile;
use crate::*;

/// What phase of evaluating a tree?
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
    outcomes: Vec<Outcome>,
}

impl LabOutcome {
    /// Record the event of one test.
    pub fn add(&mut self, outcome: &Outcome) {
        self.outcomes.push(outcome.clone());
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
        } else if self.outcomes.iter().any(|o| o.has_timeout()) {
            exit_code::TIMEOUT
        } else if self.outcomes.iter().any(|o| o.mutant_missed()) {
            exit_code::FOUND_PROBLEMS
        } else {
            exit_code::SUCCESS
        }
    }
}

/// The result of running one mutation scenario.
#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[must_use]
pub struct Outcome {
    /// A file holding the text output from running this test.
    log_path: PathBuf,
    /// What kind of scenario was being built?
    pub scenario: Scenario,
    /// For each phase, the duration and the cargo result.
    phase_results: Vec<PhaseResult>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
struct PhaseResult {
    phase: Phase,
    duration: Duration,
    cargo_result: CargoResult,
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
}

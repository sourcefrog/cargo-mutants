// Copyright 2022 Martin Pool

//! The outcome of running a command.

use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Context;

use crate::exit_code;
use crate::log_file::LogFile;
use crate::Result;

/// What type of build, check, or test was this?
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum Scenario {
    /// Build in the original source tree.
    SourceTree,
    /// Build in a copy of the source tree but with no mutations applied.
    Baseline,
    /// Build with a mutant applied.
    Mutant,
}

/// The result of running a single Cargo command.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CargoResult {
    // Note: This is not, for now, a Result, because it seems like there is
    // no clear "normal" success: sometimes a non-zero exit is what we want, etc.
    // They seem to be all on the same level as far as how the caller should respond.
    // However, failing to even start Cargo is simply an Error, and should
    // probably stop the cargo-mutants job.
    Timeout,
    Success,
    Failure,
}

impl CargoResult {
    pub fn success(&self) -> bool {
        matches!(self, CargoResult::Success)
    }
}

/// What phase of evaluating a tree?
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
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
#[derive(Debug, Default)]
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
        use CargoResult::*;
        use Scenario::*;
        if self
            .outcomes
            .iter()
            .any(|o| matches!(o.scenario, SourceTree | Baseline) && !o.cargo_result.success())
        {
            exit_code::CLEAN_TESTS_FAILED
        } else if self.outcomes.iter().any(|o| o.cargo_result == Timeout) {
            exit_code::TIMEOUT
        } else if self.outcomes.iter().any(|o| {
            matches!(
                o,
                Outcome {
                    scenario: Mutant,
                    cargo_result: Success,
                    phase: Phase::Test,
                    ..
                }
            )
        }) {
            exit_code::FOUND_PROBLEMS
        } else {
            exit_code::SUCCESS
        }
    }
}

/// The result of running one mutation scenario.
#[derive(Debug, Clone, Eq, PartialEq)]
#[must_use]
pub struct Outcome {
    /// A file holding the text output from running this test.
    log_path: PathBuf,
    pub duration: Duration,
    /// What kind of scenario was being built?
    pub scenario: Scenario,
    pub cargo_result: CargoResult,
    pub phase: Phase,
}

impl Outcome {
    pub fn new(
        log_file: &LogFile,
        start_time: &Instant,
        scenario: Scenario,
        cargo_result: CargoResult,
        phase: Phase,
    ) -> Outcome {
        Outcome {
            log_path: log_file.path().to_owned(),
            duration: start_time.elapsed(),
            scenario,
            cargo_result,
            phase,
        }
    }

    pub fn get_log_content(&self) -> Result<String> {
        fs::read_to_string(&self.log_path).context("read log file")
    }

    /// True if this status indicates the user definitely needs to see the logs, because a task
    /// failed that should not have.
    pub fn should_show_logs(&self) -> bool {
        self.scenario != Scenario::Mutant && !self.cargo_result.success()
    }
}

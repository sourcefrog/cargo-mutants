// Copyright 2021 Martin Pool

//! Representation of the outcome of a test, or a whole lab.

use std::collections::HashMap;
use std::process;
use std::time::Duration;

use crate::exit_code;
use crate::output::LogFile;

/// All the data from running one test.
#[derive(Debug)]
#[must_use]
pub struct Outcome {
    /// High-level categorization of what happened.
    pub status: Status,
    /// A file holding the text output from running this test.
    pub log_file: LogFile,
    pub duration: Duration,
}

/// The outcome from running a group of tests.
#[derive(Debug, Default)]
pub struct LabOutcome {
    count_by_status: HashMap<Status, usize>,
}

impl LabOutcome {
    /// Record the event of one test.
    pub fn add(&mut self, outcome: &Outcome) {
        self.count_by_status
            .entry(outcome.status)
            .and_modify(|c| *c += 1)
            .or_insert(1);
    }

    /// Return the count of tests that failed with the given status.
    pub fn count(&self, status: Status) -> usize {
        self.count_by_status
            .get(&status)
            .cloned()
            .unwrap_or_default()
    }

    /// Return the overall program exit code reflecting this outcome.
    pub fn exit_code(&self) -> i32 {
        use Status::*;
        if self.count(CleanTestFailed) > 0 {
            exit_code::CLEAN_TESTS_FAILED
        } else if self.count(Timeout) > 0 {
            exit_code::TIMEOUT
        } else if self.count(MutantMissed) > 0 {
            exit_code::FOUND_PROBLEMS
        } else {
            exit_code::SUCCESS
        }
    }
}

/// The bottom line of running a test: it passed, failed, timed out, etc.
// TODO: Maybe these would be better as an Error type and in the Err branch of a Result?
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
#[must_use]
pub enum Status {
    /// The mutation was caught by tests.
    MutantCaught,
    /// The mutation was not caught by any tests.
    MutantMissed,
    /// Test ran too long and was killed. Maybe the mutation caused an infinite
    /// loop.
    Timeout,
    /// The tests are already failing in a clean tree.
    CleanTestFailed,
    /// Tests passed in a clean tree.
    CleanTestPassed,
}

impl Status {
    pub fn from_mutant_test(exit_status: process::ExitStatus) -> Status {
        // TODO: Detect signals and cargo failures other than test failures.
        if exit_status.success() {
            Status::MutantMissed
        } else {
            Status::MutantCaught
        }
    }

    pub fn from_clean_test(exit_status: process::ExitStatus) -> Status {
        if exit_status.success() {
            Status::CleanTestPassed
        } else {
            Status::CleanTestFailed
        }
    }
}

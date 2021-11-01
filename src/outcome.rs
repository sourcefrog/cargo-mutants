// Copyright 2021 Martin Pool

//! Representation of the outcome of a test, or a whole lab.

use std::collections::HashMap;
use std::process;
use std::time::Duration;

use crate::exit_code;

/// All the data from running one test.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Outcome {
    /// High-level categorization of what happened.
    pub status: Status,
    // TODO: Maybe just remember the file name and load it on demand; this overlaps a bit with log file handling.
    pub log_content: String,
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
        if self.count(CleanTestsFailed) > 0 {
            exit_code::CLEAN_TESTS_FAILED
        } else if self.count(Timeout) > 0 {
            exit_code::TIMEOUT
        } else if self.count(Failed) > 0 {
            exit_code::FOUND_PROBLEMS
        } else {
            exit_code::SUCCESS
        }
    }
}

/// The bottom line of running a test: it passed, failed, timed out, etc.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum Status {
    Failed,
    /// The tests passed. This is desired in a clean tree and undesired in a
    /// mutated tree.
    Passed,
    /// Test ran too long and was killed. Maybe the mutation caused an infinite
    /// loop.
    Timeout,
    /// The tests are already failing in a clean tree.
    CleanTestsFailed,
}

impl From<process::ExitStatus> for Status {
    fn from(status: process::ExitStatus) -> Status {
        if status.success() {
            Status::Passed
        } else {
            Status::Failed
        }
    }
}

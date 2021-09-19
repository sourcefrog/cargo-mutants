// Copyright 2021 Martin Pool

use std::process;
/// The outcome from a test.
use std::time::Duration;

/// All the data from running one test.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Outcome {
    pub status: Status,
    pub stdout: String,
    pub stderr: String,
    pub duration: Duration,
}

/// The bottom line of running a test: it passed, failed, timed out, etc.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Status {
    Failed,
    /// The tests passed. This is desired in a clean tree and undesired in a
    /// mutated tree.
    Passed,
    /// Test ran too long and was killed. Maybe the mutation caused an infinite
    /// loop.
    Timeout,
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

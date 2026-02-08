// Copyright 2021 - 2025 Martin Pool

//! Exit codes from cargo-mutants.
//!
//! These are assigned so that different cases that CI or other automation (or
//! cargo-mutants' own test suite) might want to distinguish are distinct.
//!
//! These are also described in README.md.

use std::process::ExitCode as StdExitCode;

// TODO: Maybe merge this with outcome::Status, and maybe merge with sysexit.

/// Exit codes for cargo-mutants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ExitCode {
    /// Everything worked and all the mutants were caught.
    Success = 0,
    /// The wrong arguments, etc.
    ///
    /// (1 is also the value returned by Clap.)
    Usage = 1,
    /// Found one or mutants that were not caught by tests.
    FoundProblems = 2,
    /// One or more tests timed out: probably the mutant caused an infinite loop, or the timeout is too low.
    Timeout = 3,
    /// The tests are already failing in an unmutated tree.
    BaselineFailed = 4,
    /// The filter diff new text does not match the source tree content.
    FilterDiffMismatch = 5,
    /// The filter diff could not be parsed.
    FilterDiffInvalid = 6,
    /// An internal software error, from sysexit.
    Software = 70,
}

impl From<ExitCode> for StdExitCode {
    fn from(code: ExitCode) -> Self {
        // All exit codes are known to be valid u8 values
        #[allow(clippy::cast_possible_truncation)]
        StdExitCode::from(code as u8)
    }
}

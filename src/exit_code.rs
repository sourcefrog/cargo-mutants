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
pub enum ExitCode {
    /// Everything worked and all the mutants were caught.
    Success,
    /// The wrong arguments, etc.
    ///
    /// (1 is also the value returned by Clap.)
    Usage,
    /// Found one or mutants that were not caught by tests.
    FoundProblems,
    /// One or more tests timed out: probably the mutant caused an infinite loop, or the timeout is too low.
    Timeout,
    /// The tests are already failing in an unmutated tree.
    BaselineFailed,
    /// The filter diff new text does not match the source tree content.
    FilterDiffMismatch,
    /// The filter diff could not be parsed.
    FilterDiffInvalid,
    /// An internal software error, from sysexit.
    Software,
}

impl ExitCode {
    /// Returns the numeric exit code value.
    pub const fn code(self) -> i32 {
        match self {
            Self::Success => 0,
            Self::Usage => 1,
            Self::FoundProblems => 2,
            Self::Timeout => 3,
            Self::BaselineFailed => 4,
            Self::FilterDiffMismatch => 5,
            Self::FilterDiffInvalid => 6,
            Self::Software => 70,
        }
    }
}

impl From<ExitCode> for StdExitCode {
    fn from(code: ExitCode) -> Self {
        // All exit codes are known to be valid u8 values
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        StdExitCode::from(code.code() as u8)
    }
}

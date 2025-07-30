// Copyright 2021 - 2025 Martin Pool

//! Exit codes from cargo-mutants.
//!
//! These are assigned so that different cases that CI or other automation (or
//! cargo-mutants' own test suite) might want to distinguish are distinct.
//!
//! These are also described in README.md.

// TODO: Maybe merge this with outcome::Status, and maybe merge with sysexit.

/// Everything worked and all the mutants were caught.
pub const SUCCESS: i32 = 0;

/// The wrong arguments, etc.
///
/// (1 is also the value returned by Clap.)
pub const USAGE: i32 = 1;

/// Found one or mutants that were not caught by tests.
pub const FOUND_PROBLEMS: i32 = 2;

/// One or more tests timed out: probably the mutant caused an infinite loop, or the timeout is too low.
pub const TIMEOUT: i32 = 3;

/// The tests are already failing in an unmutated tree.
pub const BASELINE_FAILED: i32 = 4;

/// The filter diff new text does not match the source tree content.
pub const FILTER_DIFF_MISMATCH: i32 = 5;

/// The filter diff could not be parsed.
pub const FILTER_DIFF_INVALID: i32 = 6;

/// An internal software error, from sysexit.
pub const SOFTWARE: i32 = 70;

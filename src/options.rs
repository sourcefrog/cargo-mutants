// Copyright 2021, 2022 Martin Pool

//! Global in-process options for experimenting on mutants.

use std::time::Duration;

use crate::*;

/// Options for running experiments.
#[derive(Default, Debug)]
pub struct Options {
    /// Don't run the tests, just see if each mutant builds.
    pub check_only: bool,

    /// Maximum run time for each cargo command.
    timeout: Duration,
}

impl Options {
    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

impl From<&Args> for Options {
    fn from(args: &Args) -> Options {
        Options {
            check_only: args.check,
            timeout: args
                .timeout
                .map(Duration::from_secs_f64)
                .unwrap_or(Duration::MAX),
        }
    }
}

// Copyright 2021, 2022 Martin Pool

//! Global in-process options for experimenting on mutants.

use std::time::Duration;

use crate::*;

/// Options for running experiments.
#[derive(Default, Debug, Clone)]
pub struct Options {
    /// Don't run the tests, just see if each mutant builds.
    pub check_only: bool,

    test_timeout: Duration,
}

impl Options {
    /// Return the maximum run time for `cargo test` commands.
    ///commands
    /// Build and check are not affected.
    pub fn test_timeout(&self) -> Duration {
        self.test_timeout
    }

    pub fn has_test_timeout(&self) -> bool {
        self.test_timeout < Duration::MAX
    }

    pub fn set_test_timeout(&mut self, test_timeout: Duration) {
        self.test_timeout = test_timeout;
    }
}

impl From<&Args> for Options {
    fn from(args: &Args) -> Options {
        Options {
            check_only: args.check,
            test_timeout: args
                .timeout
                .map(Duration::from_secs_f64)
                .unwrap_or(Duration::MAX),
        }
    }
}

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

    pub print_caught: bool,
    pub print_unviable: bool,

    pub show_times: bool,

    /// Show logs even from mutants that were caught, or source/unmutated builds.
    pub show_all_logs: bool,

    /// Test mutants in random order.
    ///
    /// This is now the default, so that repeated partial runs are more likely to find
    /// interesting results.
    pub shuffle: bool,

    /// Additional arguments to `cargo test`.
    pub additional_cargo_test_args: Vec<String>,

    /// Copy the `/target/` directory from the source tree.
    pub copy_target: bool,

    /// Build the source directory before copying it.
    pub build_source: bool,
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
            build_source: !args.no_copy_target,
            check_only: args.check,
            copy_target: !args.no_copy_target,
            print_caught: args.caught,
            print_unviable: args.unviable,
            shuffle: !args.no_shuffle,
            show_times: !args.no_times,
            show_all_logs: args.all_logs,
            test_timeout: args
                .timeout
                .map(Duration::from_secs_f64)
                .unwrap_or(Duration::MAX),
            additional_cargo_test_args: args.cargo_test_args.clone(),
        }
    }
}

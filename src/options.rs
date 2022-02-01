// Copyright 2021, 2022 Martin Pool

//! Global in-process options for experimenting on mutants.

use crate::*;

/// Options for running experiments.
#[derive(Default, Debug)]
pub struct Options {
    /// Don't run the tests, just see if each mutant builds.
    pub check_only: bool,
}

impl From<&Args> for Options {
    fn from(args: &Args) -> Options {
        Options {
            check_only: args.check,
        }
    }
}

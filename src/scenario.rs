// Copyright 2021-2023 Martin Pool

use serde::Serialize;
use std::fmt;

use crate::Mutant;

/// A scenario is either a freshening build in the source tree, a baseline test with no mutations, or a mutation test.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub enum Scenario {
    /// Build in a copy of the source tree but with no mutations applied.
    Baseline,
    /// Build with a mutation applied.
    Mutant(Mutant),
}

impl fmt::Display for Scenario {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Scenario::Baseline => f.write_str("baseline"),
            Scenario::Mutant(mutant) => mutant.fmt(f),
        }
    }
}

impl Scenario {
    pub fn is_mutant(&self) -> bool {
        matches!(self, Scenario::Mutant { .. })
    }

    pub fn log_file_name_base(&self) -> String {
        match self {
            Scenario::Baseline => "baseline".into(),
            Scenario::Mutant(mutant) => mutant.log_file_name_base(),
        }
    }
}

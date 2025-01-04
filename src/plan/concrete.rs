// Copyright 2025 Martin Pool

/// A concrete command to be executed in a scenario.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Command {
    pub argv: Vec<String>,
    pub env: Vec<(String, String)>,
}

// Copyright 2021 Martin Pool

/// Print messages to the terminal.
use std::io::{stdout, Write};

use crate::lab::Outcome;

pub fn show_start(msg: &str) {
    print!("{} ... ", msg);
    stdout().flush().unwrap();
}

pub fn show_result(msg: &str) {
    println!("{}", msg);
}

pub fn show_outcome(outcome: &Outcome) {
    match outcome {
        Outcome::Caught => show_result("caught"),
        Outcome::NotCaught => show_result("NOT CAUGHT!"),
    }
}

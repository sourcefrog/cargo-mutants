// Copyright 2021 Martin Pool

/// Print messages to the terminal.
use std::io::{stdout, Write};

use console::style;

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
        Outcome::Caught => println!("{}", style("caught").green()),
        Outcome::NotCaught => println!("{}", style("NOT CAUGHT!").bold().red()),
    }
}

// Copyright 2021 Martin Pool

/// Print messages to the terminal.
use std::io::{stdout, Write};
use std::time::Duration;

use console::style;

use crate::lab::Outcome;

pub fn show_start(msg: &str) {
    print!("{} ... ", msg);
    stdout().flush().unwrap();
}

pub fn show_success(msg: &str, duration: &Duration) {
    println!("{} in {}", style(msg).green(), format_elapsed(duration));
}

pub fn show_failure(msg: &str, duration: &Duration) {
    println!(
        "{} in {}",
        style(msg).red().bold(),
        format_elapsed(duration)
    );
}

pub fn show_outcome(outcome: &Outcome, duration: &Duration) {
    match outcome {
        Outcome::Caught => show_success("caught", duration),
        Outcome::NotCaught => show_failure("NOT CAUGHT", duration),
    }
}

fn format_elapsed(duration: &Duration) -> String {
    format!("{:.3}s", duration.as_secs_f64())
}

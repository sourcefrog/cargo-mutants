// Copyright 2021 Martin Pool

/// Print messages to the terminal.
use std::io::{stdout, Write};
use std::time::Duration;

use console::style;

use crate::outcome::{Outcome, Status};

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

pub fn show_outcome(outcome: &Outcome) {
    match outcome.status {
        Status::Failed => show_success("caught", &outcome.duration),
        Status::Passed => show_failure("NOT CAUGHT", &outcome.duration),
        Status::Timeout => show_failure("TIMEOUT", &outcome.duration),
        // OutcomeType::Timeout => show_failure("TIMEOUT", duration),
    }
}

pub fn show_baseline_outcome(outcome: &Outcome) {
    match outcome.status {
        Status::Passed => {
            show_success("ok", &outcome.duration);
        }
        Status::Failed | Status::Timeout => {
            show_failure(&format!("{:?}", outcome.status), &outcome.duration);
            // println!("error: baseline tests in clean tree failed; tests won't continue");
            print!("{}", &outcome.stdout);
            print!("{}", &outcome.stderr);
        }
    }
}

fn format_elapsed(duration: &Duration) -> String {
    format!("{:.3}s", duration.as_secs_f64())
}

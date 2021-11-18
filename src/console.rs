// Copyright 2021 Martin Pool

//! Print messages and progress bars on the terminal.

use std::time::Instant;

use anyhow::Result;
use console::{style, StyledObject};
use indicatif::{ProgressBar, ProgressStyle};

use crate::lab::{Outcome, Status};
use crate::mutate::Mutation;

/// Top-level UI object that manages the state of an interactive console: mostly progress bars and
/// messages.
pub struct Console {
    pub show_all_logs: bool,
}

impl Console {
    /// Construct a new rich text UI.
    pub fn new(show_all_logs: bool) -> Console {
        Console { show_all_logs }
    }

    /// Create an Activity for a new mutation.
    pub fn start_mutation(&self, mutation: &Mutation) -> Activity {
        self.start_activity(&style_mutation(mutation))
    }

    /// Start a general-purpose activity.
    pub fn start_activity(&self, task: &str) -> Activity {
        let progress_bar = ProgressBar::new(0)
            .with_message(task.to_owned())
            .with_style(
                ProgressStyle::default_spinner()
                    .template("{msg} ... {elapsed:.cyan} {spinner:.cyan}"),
            );
        progress_bar.set_draw_rate(5); // updates per second
        Activity {
            task: task.to_owned(),
            progress_bar,
            start_time: Instant::now(),
            show_all_logs: self.show_all_logs,
        }
    }
}

pub struct Activity {
    pub start_time: Instant,
    progress_bar: ProgressBar,
    task: String,
    show_all_logs: bool,
}

impl Activity {
    pub fn set_phase(&mut self, phase: &'static str) {
        self.progress_bar
            .set_message(format!("{} ({})", self.task, phase));
    }

    pub fn succeed(self, msg: &str) {
        self.finish(style(msg).green());
    }

    pub fn fail(self, msg: &str) {
        self.finish(style(msg).bold().red());
    }

    /// Finish the progress bar, and print a concluding message to stdout.
    fn finish(self, styled_status: StyledObject<&str>) {
        self.progress_bar.finish_and_clear();
        println!(
            "{} ... {} in {}",
            self.task,
            styled_status,
            self.format_elapsed()
        );
    }

    pub fn tick(&mut self) {
        self.progress_bar.tick();
    }

    pub fn tick_message(&mut self, message: &str) {
        self.progress_bar
            .set_message(format!("{}... {}", self.task, message));
        self.progress_bar.tick();
    }

    /// Report the outcome of a scenario.
    ///
    /// Prints the log content if appropriate.
    pub fn outcome(self, outcome: &Outcome) -> Result<()> {
        let show_all_logs = self.show_all_logs; // survive consumption by finish
        self.finish(style_status(outcome.status));
        if outcome.status == Status::CleanTestFailed || show_all_logs {
            print!("{}", outcome.log_file.log_content()?);
        }
        Ok(())
    }

    fn format_elapsed(&self) -> String {
        format!("{:.3}s", &self.start_time.elapsed().as_secs_f64())
    }
}

/// Return a styled string reflecting the moral value of this outcome.
pub fn style_status(status: Status) -> StyledObject<&'static str> {
    use Status::*;
    match status {
        // good statuses
        MutantCaught => style("caught").green(),
        CleanTestPassed => style("ok").green(),
        // neutral/inconclusive
        CheckFailed => style("check failed").yellow().bold(),
        BuildFailed => style("build failed").yellow().bold(),
        // bad statuses
        MutantMissed => style("NOT CAUGHT").red().bold(),
        Timeout => style("TIMEOUT").red().bold(),
        CleanTestFailed => style("FAILED").red().bold(),
    }
}

pub fn list_mutations(mutations: &[Mutation], show_diffs: bool) {
    for mutation in mutations {
        println!("{}", style_mutation(mutation));
        if show_diffs {
            println!("{}", mutation.diff());
        }
    }
}

fn style_mutation(mutation: &Mutation) -> String {
    format!(
        "{}: replace {} with {}",
        mutation.describe_location(),
        style(mutation.function_name()).bright().magenta(),
        style(mutation.replacement_text()).yellow(),
    )
}

pub fn print_error(msg: &str) {
    println!("{}: {}", style("error").bold().red(), msg);
}

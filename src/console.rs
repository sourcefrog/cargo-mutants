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

    /// Start an Activity for copying a tree.
    pub fn start_copy_activity(&self, name: &str) -> CopyActivity {
        CopyActivity::new(name)
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

    /// Report the outcome of a scenario.
    ///
    /// Prints the log content if appropriate.
    pub fn outcome(self, outcome: &Outcome) -> Result<()> {
        let show_all_logs = self.show_all_logs; // survive consumption by finish
        self.finish(style_status(outcome.status));
        if outcome.status.should_show_logs() || show_all_logs {
            print!("{}", outcome.log_file.log_content()?);
        }
        Ok(())
    }

    fn format_elapsed(&self) -> String {
        format_elapsed(self.start_time)
    }
}

pub struct CopyActivity {
    name: String,
    progress_bar: ProgressBar,
    start_time: Instant,
}

impl CopyActivity {
    fn new(name: &str) -> CopyActivity {
        let progress_bar = ProgressBar::new(0)
            .with_message(name.to_owned())
            .with_style(ProgressStyle::default_spinner().template("{msg}"));
        progress_bar.set_draw_rate(5); // updates per second
        CopyActivity {
            name: name.to_owned(),
            progress_bar,
            start_time: Instant::now(),
        }
    }

    pub fn bytes_copied(&mut self, bytes_copied: u64) {
        self.progress_bar.set_message(self.format(bytes_copied))
    }

    pub fn succeed(self, bytes_copied: u64) {
        self.progress_bar.finish_and_clear();
        // Print to stdout even if progress bars weren't drawn.
        println!("{}", self.format(bytes_copied));
    }

    pub fn fail(self) {
        self.progress_bar.finish_and_clear();
        println!("{} ... {}", self.name, style("failed").bold().red(),);
    }

    fn format(&self, bytes_copied: u64) -> String {
        format!(
            "{} ... {} in {}",
            self.name,
            style_mb(bytes_copied),
            style(format_elapsed(self.start_time)).cyan(),
        )
    }
}

/// Return a styled string reflecting the moral value of this outcome.
pub fn style_status(status: Status) -> StyledObject<&'static str> {
    use Status::*;
    match status {
        // good statuses
        MutantCaught => style("caught").green(),
        SourceBuildPassed | CleanTestPassed => style("ok").green(),
        // neutral/inconclusive
        CheckFailed => style("check failed").yellow(),
        BuildFailed => style("build failed").yellow(),
        // bad statuses
        MutantMissed => style("NOT CAUGHT").red().bold(),
        Timeout => style("TIMEOUT").red().bold(),
        SourceBuildFailed | CleanTestFailed => style("FAILED").red().bold(),
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
        "{}: replace {}{}{} with {}",
        mutation.describe_location(),
        style(mutation.function_name()).bright().magenta(),
        if mutation.return_type().is_empty() {
            ""
        } else {
            " "
        },
        style(mutation.return_type()).magenta(),
        style(mutation.replacement_text()).yellow(),
    )
}

pub fn print_error(msg: &str) {
    println!("{}: {}", style("error").bold().red(), msg);
}

fn format_elapsed(since: Instant) -> String {
    format!("{:.3}s", since.elapsed().as_secs_f64())
}

fn format_mb(bytes: u64) -> String {
    format!("{} MB", bytes / 1_000_000)
}

fn style_mb(bytes: u64) -> StyledObject<String> {
    style(format_mb(bytes)).cyan()
}

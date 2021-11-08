// Copyright 2021 Martin Pool

//! Print messages and progress bars on the terminal.

use std::time::Instant;

use console::style;
use indicatif::{ProgressBar, ProgressStyle};

use crate::mutate::Mutation;
use crate::outcome::{Outcome, Status};

pub(crate) struct Activity {
    pub start_time: Instant,
    progress_bar: ProgressBar,
    task: String,
}

impl Activity {
    pub fn start(msg: &str) -> Activity {
        let progress_bar = ProgressBar::new(0).with_message(msg.to_owned()).with_style(
            ProgressStyle::default_spinner().template("{msg} ... {elapsed:.cyan} {spinner:.cyan}"),
        );
        Activity {
            task: msg.to_owned(),
            progress_bar,
            start_time: Instant::now(),
        }
    }

    pub fn start_mutation(mutation: &Mutation) -> Activity {
        Activity::start(&style_mutation(mutation))
    }

    pub fn succeed(self, msg: &str) {
        self.progress_bar.finish_and_clear();
        // The progress bar was drawn to stdout but this message goes to stdout even if it's not a tty.
        println!(
            "{} ... {} in {}",
            self.task,
            style(msg).green(),
            self.format_elapsed()
        );
    }

    pub fn fail(self, msg: &str) {
        self.progress_bar.finish_and_clear();
        println!(
            "{} ... {} in {}",
            self.task,
            style(msg).red().bold(),
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

    pub fn outcome(self, outcome: &Outcome) {
        use Status::*;
        match outcome.status {
            MutantCaught => self.succeed("caught"),
            MutantMissed => self.fail("NOT CAUGHT"),
            Timeout => self.fail("TIMEOUT"),
            CleanTestFailed => self.fail("FAILED"),
            CleanTestPassed => self.succeed("ok"),
        }
    }

    fn format_elapsed(&self) -> String {
        format!("{:.3}s", &self.start_time.elapsed().as_secs_f64())
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

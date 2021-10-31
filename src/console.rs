// Copyright 2021 Martin Pool

//! Print messages to the terminal.

use std::io::{stdout, Write};
use std::time::Instant;

use atty::Stream;
use console::style;

use crate::mutate::Mutation;
use crate::outcome::{Outcome, Status};

pub(crate) struct Activity {
    pub start_time: Instant,
    atty: bool,
}

impl Activity {
    pub fn start(msg: &str) -> Activity {
        print!("{} ... ", msg);
        stdout().flush().unwrap();
        Activity {
            start_time: Instant::now(),
            atty: atty::is(Stream::Stdout),
        }
    }

    pub fn start_mutation(mutation: &Mutation) -> Activity {
        Activity::start(&style_mutation(mutation))
    }

    pub fn succeed(self, msg: &str) {
        println!("{} in {}", style(msg).green(), self.format_elapsed());
    }

    pub fn fail(self, msg: &str) {
        println!("{} in {}", style(msg).red().bold(), self.format_elapsed());
    }

    pub fn tick(&self) {
        if self.atty {
            let time_str = format!("{}s", self.start_time.elapsed().as_secs());
            let backspace = "\x08".repeat(time_str.len());
            print!("{}{}", time_str, backspace);
            stdout().flush().unwrap();
        }
    }

    pub fn outcome(self, outcome: &Outcome) {
        match outcome.status {
            Status::Failed => self.succeed("caught"),
            Status::Passed => self.fail("NOT CAUGHT"),
            Status::Timeout => self.fail("TIMEOUT"),
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

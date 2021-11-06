// Copyright 2021 Martin Pool

//! Print messages to the terminal.

use std::io::{stdout, Write};
use std::time::{Duration, Instant};

use atty::Stream;
use console::style;

use crate::mutate::Mutation;
use crate::outcome::{Outcome, Status};

pub(crate) struct Activity {
    pub start_time: Instant,
    atty: bool,
    last_tick: Instant,
}

impl Activity {
    pub fn start(msg: &str) -> Activity {
        print!("{} ... ", msg);
        stdout().flush().unwrap();
        Activity {
            start_time: Instant::now(),
            atty: atty::is(Stream::Stdout),
            last_tick: Instant::now(),
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

    pub fn tick(&mut self) {
        self.tick_message("");
    }

    pub fn tick_message(&mut self, message: &str) {
        let now = Instant::now();
        if self.atty && now.duration_since(self.last_tick) > Duration::from_millis(100) {
            self.last_tick = now;
            let mut buf = format!("{}s", self.start_time.elapsed().as_secs());
            if !message.is_empty() {
                use std::fmt::Write;
                write!(buf, " {}", message).unwrap();
            }
            let backspace = "\x08".repeat(buf.len());
            print!("{}{}", buf, backspace);
            stdout().flush().unwrap();
        }
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

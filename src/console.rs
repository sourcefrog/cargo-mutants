// Copyright 2021 Martin Pool

/// Print messages to the terminal.
use std::io::{stdout, Write};
use std::time::{Duration, Instant};

use console::style;

use crate::outcome::{Outcome, Status};

pub(crate) struct Activity {
    pub start_time: Instant,
}

impl Activity {
    pub fn start(msg: &str) -> Activity {
        print!("{} ... ", msg);
        stdout().flush().unwrap();
        Activity {
            start_time: Instant::now(),
        }
    }

    pub fn succeed(self, msg: &str) {
        println!("{} in {}", style(msg).green(), self.format_elapsed());
    }

    pub fn fail(self, msg: &str) {
        println!("{} in {}", style(msg).red().bold(), self.format_elapsed());
    }

    pub fn outcome(self, outcome: &Outcome) {
        match outcome.status {
            Status::Failed => self.succeed("caught"),
            Status::Passed => self.fail("NOT CAUGHT"),
            Status::Timeout => self.fail("TIMEOUT"),
        }
    }

    fn format_elapsed(&self) -> String {
        format_elapsed(&self.start_time.elapsed())
    }
}

fn format_elapsed(duration: &Duration) -> String {
    format!("{:.3}s", duration.as_secs_f64())
}

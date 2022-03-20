// Copyright 2021, 2022 Martin Pool

//! Print messages and progress bars on the terminal.

use std::time::Instant;

use ::console::{style, StyledObject};
use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};

use crate::lab::Scenario;
use crate::mutate::Mutation;
use crate::outcome::{Outcome, Phase};
use crate::*;

/// Top-level UI object that manages the state of an interactive console: mostly progress bars and
/// messages.
pub struct Console {
    show_times: bool,
}

impl Console {
    /// Construct a new rich text UI.
    pub fn new(options: &Options) -> Console {
        Console {
            show_times: options.show_times,
        }
    }

    pub fn start_scenario(&self, scenario: &Scenario) -> BuildActivity {
        match scenario {
            Scenario::SourceTree => BuildActivity::new("source tree", self.show_times),
            Scenario::Baseline => BuildActivity::new("unmutated baseline", self.show_times),
            Scenario::Mutant {
                mutation,
                i_mutation,
                n_mutations,
            } => {
                let mut activity = BuildActivity::new(style_mutation(mutation), self.show_times);
                activity.overall_progress = Some((i_mutation + 1, *n_mutations));
                activity
            }
        }
    }
}

pub struct BuildActivity {
    pub start_time: Instant,
    progress_bar: ProgressBar,
    task: String,
    show_times: bool,
    /// Optionally, progress counter through the overall lab. Shown in the progress bar
    /// but not on permanent output.
    overall_progress: Option<(usize, usize)>,
}

impl BuildActivity {
    /// Start a general-purpose activity.
    fn new<S: Into<String>>(task: S, show_times: bool) -> BuildActivity {
        let task = task.into();
        let progress_bar = ProgressBar::new(0).with_message(task.clone()).with_style(
            ProgressStyle::default_spinner().template("{msg} ... {elapsed:.cyan} {spinner:.cyan}"),
        );
        progress_bar.set_draw_rate(5); // updates per second
        BuildActivity {
            show_times,
            task,
            progress_bar,
            start_time: Instant::now(),
            overall_progress: None,
        }
    }

    pub fn set_phase(&mut self, phase: &'static str) {
        let overall_text = self
            .overall_progress
            .map_or(String::new(), |(a, b)| format!("[{}/{}] ", a, b));
        self.progress_bar
            .set_message(format!("{}{} ({})", overall_text, self.task, phase));
    }

    /// Mark this activity as interrupted.
    pub fn interrupted(&mut self) {
        self.progress_bar.finish_and_clear();
        println!("{} ... {}", self.task, style("interrupted").bold().red());
    }

    pub fn tick(&mut self) {
        self.progress_bar.tick();
    }

    /// Report the outcome of a scenario.
    ///
    /// Prints the log content if appropriate.
    pub fn outcome(self, outcome: &Outcome, options: &Options) -> Result<()> {
        self.progress_bar.finish_and_clear();
        if (outcome.mutant_caught() && !options.print_caught)
            || (outcome.scenario.is_mutant()
                && outcome.check_or_build_failed()
                && !options.print_unviable)
        {
            return Ok(());
        }

        print!("{} ... {}", self.task, style_outcome(outcome));
        if self.show_times {
            println!(" in {}", self.format_elapsed());
        } else {
            println!();
        }
        if outcome.should_show_logs() || options.show_all_logs {
            print!("{}", outcome.get_log_content()?);
        }
        Ok(())
    }

    fn format_elapsed(&self) -> String {
        format_elapsed(self.start_time)
    }
}

pub struct CopyActivity {
    view: nutmeg::View<CopyModel>,
}

struct CopyModel {
    bytes_copied: u64,
    start: Instant,
    name: &'static str,
    succeeded: bool,
    show_times: bool,
}

impl nutmeg::Model for CopyModel {
    fn render(&mut self, _width: usize) -> String {
        format!(
            "{} ... {} in {}",
            self.name,
            style_mb(self.bytes_copied),
            style(format!("{}s", self.start.elapsed().as_secs())).cyan()
        )
    }

    fn final_message(&mut self) -> String {
        if self.succeeded {
            if self.show_times {
                format!(
                    "{} ... {} in {}",
                    self.name,
                    style_mb(self.bytes_copied),
                    style(format_elapsed(self.start)).cyan(),
                )
            } else {
                format!("{} ... {}", self.name, style("done").green())
            }
        } else {
            format!("{} ... {}", self.name, style("failed").bold().red())
        }
    }
}

impl CopyActivity {
    pub fn new(name: &'static str, options: Options) -> CopyActivity {
        let view = nutmeg::View::new(
            CopyModel {
                name,
                start: Instant::now(),
                bytes_copied: 0,
                succeeded: false,
                show_times: options.show_times,
            },
            nutmeg::Options::default(),
        );
        CopyActivity { view }
    }

    pub fn bytes_copied(&mut self, bytes_copied: u64) {
        self.view.update(|model| model.bytes_copied = bytes_copied);
    }

    pub fn succeed(self, bytes_copied: u64) {
        self.view.update(|model| {
            model.succeeded = true;
            model.bytes_copied = bytes_copied;
        });
        self.view.finish();
    }

    pub fn fail(self) {
        self.view.finish();
    }
}

/// Return a styled string reflecting the moral value of this outcome.
pub fn style_outcome(outcome: &Outcome) -> StyledObject<&'static str> {
    use CargoResult::*;
    use Scenario::*;
    match &outcome.scenario {
        SourceTree | Baseline => match outcome.last_phase_result() {
            Success => style("ok").green(),
            Failure => style("FAILED").red().bold(),
            Timeout => style("TIMEOUT").red().bold(),
        },
        Mutant { .. } => match (outcome.last_phase(), outcome.last_phase_result()) {
            (Phase::Test, Failure) => style("caught").green(),
            (Phase::Test, Success) => style("NOT CAUGHT").red().bold(),
            (Phase::Build, Success) => style("build ok").green(),
            (Phase::Check, Success) => style("check ok").green(),
            (Phase::Build, Failure) => style("build failed").yellow(),
            (Phase::Check, Failure) => style("check failed").yellow(),
            (_, Timeout) => style("TIMEOUT").red().bold(),
        },
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

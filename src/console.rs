// Copyright 2021, 2022 Martin Pool

//! Print messages and progress bars on the terminal.

use std::borrow::Cow;
use std::fmt::Write;
use std::time::Instant;

use ::console::{style, StyledObject};
use anyhow::Result;

use crate::lab::Scenario;
use crate::mutate::Mutation;
use crate::outcome::{Outcome, Phase};
use crate::*;

pub struct CargoActivity {
    view: nutmeg::View<CargoModel>,
}

struct CargoModel {
    task: Cow<'static, str>,
    options: Options,
    start: Instant,
    phase: Option<&'static str>,

    /// Optionally, progress counter through the overall lab. Shown in the progress bar
    /// but not on permanent output.
    overall_progress: Option<(usize, usize)>,

    outcome: Option<Outcome>,
    interrupted: bool,
}

impl nutmeg::Model for CargoModel {
    fn render(&mut self, _width: usize) -> String {
        let mut s = String::new();
        if let Some((i, n)) = self.overall_progress {
            write!(s, "[{}/{}] ", i, n).unwrap();
        }
        write!(s, "{} ", self.task,).unwrap();
        if let Some(phase) = self.phase {
            write!(s, "({}) ", phase).unwrap();
        }
        write!(s, "... {}", format_elapsed_secs(self.start)).unwrap();
        s
    }

    fn final_message(&mut self) -> String {
        let mut s = String::with_capacity(100);
        if self.interrupted {
            write!(s, "{} ... {}", self.task, style("interrupted").bold().red()).unwrap();
        } else if let Some(outcome) = self.outcome.as_ref() {
            if (outcome.mutant_caught() && !self.options.print_caught)
                || (outcome.scenario.is_mutant()
                    && outcome.check_or_build_failed()
                    && !self.options.print_unviable)
            {
                return s;
            }
            write!(s, "{} ... {}", self.task, style_outcome(outcome)).unwrap();
            if self.options.show_times {
                write!(s, " in {}", format_elapsed_millis(self.start)).unwrap();
            }
        }
        s
    }
}

impl CargoActivity {
    pub fn for_scenario(scenario: &Scenario, options: &Options) -> CargoActivity {
        let options = options.clone();
        match scenario {
            Scenario::SourceTree => CargoActivity::new("source tree", options, None),
            Scenario::Baseline => CargoActivity::new("unmutated baseline", options, None),
            Scenario::Mutant {
                mutation,
                i_mutation,
                n_mutations,
            } => CargoActivity::new(
                style_mutation(mutation),
                options,
                Some((i_mutation + 1, *n_mutations)),
            ),
        }
    }

    /// Start a general-purpose activity.
    fn new<S: Into<Cow<'static, str>>>(
        task: S,
        options: Options,
        overall_progress: Option<(usize, usize)>,
    ) -> CargoActivity {
        let task = task.into();
        let model = CargoModel {
            task,
            options,
            start: Instant::now(),
            overall_progress,
            phase: None,
            outcome: None,
            interrupted: false,
        };
        CargoActivity {
            view: nutmeg::View::new(model, nutmeg_options()),
        }
    }

    pub fn set_phase(&mut self, phase: &'static str) {
        self.view.update(|model| model.phase = Some(phase));
    }

    /// Mark this activity as interrupted.
    pub fn interrupted(&mut self) {
        // TODO: Unify with outcomes?
        self.view.update(|model| model.interrupted = true);
    }

    pub fn tick(&mut self) {
        self.view.update(|_| ());
    }

    /// Report the outcome of a scenario.
    ///
    /// Prints the log content if appropriate.
    pub fn outcome(self, outcome: &Outcome, options: &Options) -> Result<()> {
        self.view
            .update(|model| model.outcome = Some(outcome.clone()));
        self.view.finish();

        if (outcome.mutant_caught() && !options.print_caught)
            || (outcome.scenario.is_mutant()
                && outcome.check_or_build_failed()
                && !options.print_unviable)
        {
            return Ok(());
        }
        if outcome.should_show_logs() || options.show_all_logs {
            print!("{}", outcome.get_log_content()?);
        }
        Ok(())
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
            format_elapsed_secs(self.start),
        )
    }

    fn final_message(&mut self) -> String {
        if self.succeeded {
            if self.show_times {
                format!(
                    "{} ... {} in {}",
                    self.name,
                    style_mb(self.bytes_copied),
                    style(format_elapsed_millis(self.start)).cyan(),
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
            nutmeg_options(),
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

fn nutmeg_options() -> nutmeg::Options {
    nutmeg::Options::default()
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

fn format_elapsed_secs(since: Instant) -> String {
    style(format!("{}s", since.elapsed().as_secs()))
        .cyan()
        .to_string()
}

fn format_elapsed_millis(since: Instant) -> String {
    format!("{:.3}s", since.elapsed().as_secs_f64())
}

fn format_mb(bytes: u64) -> String {
    format!("{} MB", bytes / 1_000_000)
}

fn style_mb(bytes: u64) -> StyledObject<String> {
    style(format_mb(bytes)).cyan()
}

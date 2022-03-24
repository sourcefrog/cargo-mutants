// Copyright 2021, 2022 Martin Pool

//! Print messages and progress bars on the terminal.
//!
//! This is modeled as a series of "activities" that each interface to the actual
//! terminal-drawing in Nutmeg.

use std::borrow::Cow;
use std::fmt::Write;
use std::sync::Arc;
use std::time::Instant;

use ::console::{style, StyledObject};
use anyhow::Result;

use crate::lab::Scenario;
use crate::mutate::Mutation;
use crate::outcome::{Outcome, Phase};
use crate::*;

/// Overall "run a bunch of experiments activity".
pub struct LabActivity {
    view: Arc<nutmeg::View<LabModel>>,
    options: Options,
}

impl LabActivity {
    pub fn new(options: &Options) -> LabActivity {
        let model = LabModel {
            lab_start: None,
            i_mutant: 0,
            n_mutants: 0,
            copy_model: None,
            cargo_model: None,
        };
        LabActivity {
            options: options.clone(),
            view: Arc::new(nutmeg::View::new(model, nutmeg_options())),
        }
    }

    pub fn start_mutants(&mut self, n_mutants: usize) {
        self.view.update(|model| {
            model.n_mutants = n_mutants;
            model.lab_start = Some(Instant::now());
        })
    }

    pub fn start_scenario(&mut self, scenario: &Scenario) -> CargoActivity {
        let cargo_model = CargoModel::new(scenario, self.options.clone());
        let task = cargo_model.task.clone();
        if let Scenario::Mutant { i_mutation, .. } = scenario {
            self.view.update(|model| model.i_mutant = *i_mutation);
        }
        self.view
            .update(|model| model.cargo_model = Some(cargo_model));
        CargoActivity {
            lab_view: self.view.clone(),
            task,
        }
    }
}

/// Description of all current activities in the lab.
///
/// At the moment there is either a copy, cargo runs, or nothing.
/// Later, there might be concurrent activities.
struct LabModel {
    copy_model: Option<CopyModel>,
    cargo_model: Option<CargoModel>,
    lab_start: Option<Instant>,
    i_mutant: usize,
    n_mutants: usize,
}

impl nutmeg::Model for LabModel {
    fn render(&mut self, width: usize) -> String {
        let mut s = String::with_capacity(100);
        if let Some(copy) = self.copy_model.as_mut() {
            s.push_str(&copy.render(width));
        }
        if let Some(cargo_model) = self.cargo_model.as_mut() {
            if !s.is_empty() {
                s.push('\n')
            }
            if let Some(lab_start) = self.lab_start {
                write!(
                    s,
                    "Trying mutant {}/{}, {} done, {} remaining\n",
                    self.i_mutant,
                    self.n_mutants,
                    nutmeg::percent_done(self.i_mutant, self.n_mutants),
                    nutmeg::estimate_remaining(&lab_start, self.i_mutant, self.n_mutants)
                )
                .unwrap();
            }
            s.push_str(&cargo_model.render(width));
        }
        s
    }
}

impl LabModel {
    fn cargo_model(&mut self) -> &mut CargoModel {
        self.cargo_model.as_mut().unwrap()
    }
}

pub struct CargoActivity {
    lab_view: Arc<nutmeg::View<LabModel>>,
    task: Cow<'static, str>,
}

struct CargoModel {
    task: Cow<'static, str>,
    options: Options,
    start: Instant,
    phase: Option<&'static str>,
}

impl nutmeg::Model for CargoModel {
    fn render(&mut self, _width: usize) -> String {
        let mut s = String::with_capacity(100);
        write!(s, "{} ", self.task,).unwrap();
        if let Some(phase) = self.phase {
            write!(s, "({}) ", phase).unwrap();
        }
        write!(s, "... {}", format_elapsed_secs(self.start)).unwrap();
        s
    }
}

impl CargoModel {
    fn new(scenario: &Scenario, options: Options) -> CargoModel {
        let task: Cow<'static, str> = match scenario {
            Scenario::SourceTree => "Freshen source tree".into(),
            Scenario::Baseline => "Unmutated baseline".into(),
            Scenario::Mutant { mutation, .. } => style_mutation(mutation).into(),
        };
        CargoModel {
            task,
            options,
            start: Instant::now(),
            phase: None,
        }
    }
}

impl CargoActivity {
    pub fn set_phase(&mut self, phase: &'static str) {
        self.lab_view
            .update(|lab_model| lab_model.cargo_model().phase = Some(phase));
    }

    /// Mark this activity as interrupted.
    pub fn interrupted(&mut self) {
        // TODO: Unify with outcomes?
        self.lab_view.update(|lab_model| {
            lab_model.cargo_model.take();
        });
        self.lab_view.message(format!(
            "{} ... {}",
            self.task,
            style("interrupted").bold().red()
        ));
    }

    pub fn tick(&mut self) {
        self.lab_view.update(|_| ());
    }

    /// Report the outcome of a scenario.
    ///
    /// Prints the log content if appropriate.
    pub fn outcome(self, outcome: &Outcome, options: &Options) -> Result<()> {
        let cargo_model = self
            .lab_view
            .update(|model| model.cargo_model.take())
            .unwrap();

        let mut s = String::with_capacity(100);
        if (outcome.mutant_caught() && !cargo_model.options.print_caught)
            || (outcome.scenario.is_mutant()
                && outcome.check_or_build_failed()
                && !cargo_model.options.print_unviable)
        {
            return Ok(());
        }
        write!(s, "{} ... {}", cargo_model.task, style_outcome(outcome)).unwrap();
        if cargo_model.options.show_times {
            write!(s, " in {}", format_elapsed_millis(cargo_model.start)).unwrap();
        }

        if outcome.should_show_logs() || options.show_all_logs {
            s.push('\n');
            write!(s, "{}", outcome.get_log_content()?).unwrap();
        }
        s.push('\n');
        self.lab_view.message(&s);
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

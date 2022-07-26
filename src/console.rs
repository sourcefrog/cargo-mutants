// Copyright 2021, 2022 Martin Pool

//! Print messages and progress bars on the terminal.
//!
//! This is modeled as a series of "activities" that each interface to the actual
//! terminal-drawing in Nutmeg.

use std::borrow::Cow;
use std::fmt::Write;
use std::time::{Duration, Instant};

use ::console::{style, StyledObject};
use camino::Utf8Path;

use crate::*;

/// Description of all current activities in the lab.
///
/// At the moment there is either a copy, cargo runs, or nothing.  Later, there
/// might be concurrent activities.
#[derive(Default)]
pub struct LabModel {
    copy_model: Option<CopyModel>,
    cargo_model: Option<CargoModel>,
    lab_start_time: Option<Instant>,
    i_mutant: usize,
    n_mutants: usize,
    mutants_caught: usize,
    mutants_missed: usize,
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
            if let Some(lab_start_time) = self.lab_start_time {
                writeln!(
                    s,
                    "Trying mutant {}/{}, {} done, {} caught, {} missed, {} remaining",
                    self.i_mutant,
                    self.n_mutants,
                    nutmeg::percent_done(self.i_mutant, self.n_mutants),
                    self.mutants_caught,
                    self.mutants_missed,
                    nutmeg::estimate_remaining(&lab_start_time, self.i_mutant, self.n_mutants)
                )
                .unwrap();
            }
            s.push_str(&cargo_model.render(width));
        }
        s
    }
}

impl LabModel {
    pub fn new() -> LabModel {
        Default::default()
    }

    /// Update that work is starting on testing a given number of mutants.
    pub fn start_mutants(&mut self, n_mutants: usize) {
        self.n_mutants = n_mutants;
        self.lab_start_time = Some(Instant::now());
    }

    /// Update that a cargo task is starting.
    pub fn start_cargo(&mut self, scenario: &Scenario, log_file: &Utf8Path) {
        let start = Instant::now();
        let cargo_model = CargoModel::new(scenario, start, log_file.to_owned());
        if let Scenario::Mutant { .. } = scenario {
            self.i_mutant += 1;
        }
        self.cargo_model = Some(cargo_model);
    }

    /// Update the phase of the cargo task.
    pub fn set_cargo_phase(&mut self, phase: &Phase) {
        self.cargo_model
            .as_mut()
            .expect("cargo model already started")
            .phase = Some(phase.name());
    }
}

/// Update that cargo finished.
pub fn cargo_outcome(
    view: &nutmeg::View<LabModel>,
    scenario: &Scenario,
    start: Instant,
    outcome: &Outcome,
    options: &Options,
) {
    view.update(|model| {
        if outcome.mutant_caught() {
            model.mutants_caught += 1
        } else if outcome.mutant_missed() {
            model.mutants_missed += 1
        }
    });

    if (outcome.mutant_caught() && !options.print_caught)
        || (outcome.scenario.is_mutant()
            && outcome.check_or_build_failed()
            && !options.print_unviable)
    {
        return;
    }

    let mut s = String::with_capacity(100);
    write!(
        s,
        "{} ... {}",
        style_scenario(scenario),
        style_outcome(outcome)
    )
    .unwrap();
    if options.show_times {
        write!(s, " in {}", format_elapsed_millis(start)).unwrap();
    }
    if outcome.should_show_logs() || options.show_all_logs {
        s.push('\n');
        s.push_str(
            outcome
                .get_log_content()
                .expect("read log content")
                .as_str(),
        );
    }
    s.push('\n');
    view.message(&s);
}

/// A Nutmeg progress model for running `cargo test` etc.
///
/// It draws the command and some description of what scenario is being tested.
pub struct CargoModel {
    name: Cow<'static, str>,
    start: Instant,
    phase: Option<&'static str>,
    log_file: Utf8PathBuf,
}

impl nutmeg::Model for CargoModel {
    fn render(&mut self, _width: usize) -> String {
        let mut s = String::with_capacity(100);
        write!(s, "{} ", self.name).unwrap();
        if let Some(phase) = self.phase {
            write!(s, "({}) ", phase).unwrap();
        }
        write!(s, "... {}", format_elapsed_secs(self.start)).unwrap();
        if let Ok(last_line) = last_line(&self.log_file) {
            write!(s, "\n    {}", last_line).unwrap();
        }
        s
    }
}

impl CargoModel {
    pub fn new(scenario: &Scenario, start: Instant, log_file: Utf8PathBuf) -> CargoModel {
        CargoModel {
            name: style_scenario(scenario),
            phase: None,
            start,
            log_file,
        }
    }
}

/// A Nutmeg model for progress in copying a tree.
pub struct CopyModel {
    bytes_copied: u64,
    start: Instant,
    name: &'static str,
    succeeded: bool,
    show_times: bool,
}

impl CopyModel {
    pub fn new(name: &'static str, options: &Options) -> CopyModel {
        CopyModel {
            name,
            start: Instant::now(),
            bytes_copied: 0,
            succeeded: false,
            show_times: options.show_times,
        }
    }

    /// Update that some bytes have been copied.
    ///
    /// `bytes_copied` is the total bytes copied so far.
    pub fn bytes_copied(&mut self, bytes_copied: u64) {
        self.bytes_copied = bytes_copied
    }

    /// Update that the copy succeeded, and set the _total_ number of bytes copies.
    pub fn succeed(&mut self, total_bytes_copied: u64) {
        self.succeeded = true;
        self.bytes_copied = total_bytes_copied;
    }
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

pub fn nutmeg_options() -> nutmeg::Options {
    nutmeg::Options::default().print_holdoff(Duration::from_secs(2))
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

pub fn list_mutants(mutants: &[Mutant], show_diffs: bool) {
    for mutant in mutants {
        println!("{}", style_mutant(mutant));
        if show_diffs {
            println!("{}", mutant.diff());
        }
    }
}

fn style_mutant(mutant: &Mutant) -> String {
    format!(
        "{}: replace {}{}{} with {}",
        mutant.describe_location(),
        style(mutant.function_name()).bright().magenta(),
        if mutant.return_type().is_empty() {
            ""
        } else {
            " "
        },
        style(mutant.return_type()).magenta(),
        style(mutant.replacement_text()).yellow(),
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

pub fn style_scenario(scenario: &Scenario) -> Cow<'static, str> {
    match scenario {
        Scenario::SourceTree => "Freshen source tree".into(),
        Scenario::Baseline => "Unmutated baseline".into(),
        Scenario::Mutant(mutant) => console::style_mutant(mutant).into(),
    }
}

pub fn style_interrupted() -> String {
    format!("{}", style("interrupted\n").bold().red())
}

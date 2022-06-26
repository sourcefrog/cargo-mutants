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

use crate::*;

/// Overall "run a bunch of experiments activity".
pub struct LabActivity {
    view: Arc<nutmeg::View<LabModel>>,
}

impl LabActivity {
    pub fn new(_options: &Options) -> LabActivity {
        let model = LabModel::default();
        LabActivity {
            view: Arc::new(nutmeg::View::new(model, nutmeg_options())),
        }
    }

    pub fn start_mutants(&mut self, n_mutants: usize) {
        self.view.update(|model| {
            model.n_mutants = n_mutants;
            model.lab_start = Some(Instant::now());
        })
    }

    pub fn start_scenario(&mut self, scenario: &Scenario, log_file: Utf8PathBuf) -> CargoActivity {
        let start = Instant::now();
        let cargo_model = CargoModel::new(scenario, start, log_file);
        let name = cargo_model.name.clone();
        if let Scenario::Mutant { .. } = scenario {
            self.view.update(|model| model.i_mutant += 1);
        }
        self.view
            .update(|model| model.cargo_model = Some(cargo_model));
        CargoActivity {
            lab_view: self.view.clone(),
            name,
            start,
        }
    }
}

/// Description of all current activities in the lab.
///
/// At the moment there is either a copy, cargo runs, or nothing.
/// Later, there might be concurrent activities.
#[derive(Default)]
struct LabModel {
    copy_model: Option<CopyModel>,
    cargo_model: Option<CargoModel>,
    lab_start: Option<Instant>,
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
            if let Some(lab_start) = self.lab_start {
                writeln!(
                    s,
                    "Trying mutant {}/{}, {} done, {} caught, {} missed, {} remaining",
                    self.i_mutant,
                    self.n_mutants,
                    nutmeg::percent_done(self.i_mutant, self.n_mutants),
                    self.mutants_caught,
                    self.mutants_missed,
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
    name: Cow<'static, str>,
    start: Instant,
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
            self.name,
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
        self.lab_view.update(|model| {
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
            return Ok(());
        }

        let mut s = String::with_capacity(100);
        write!(s, "{} ... {}", self.name, style_outcome(outcome)).unwrap();
        if options.show_times {
            write!(s, " in {}", format_elapsed_millis(self.start)).unwrap();
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

/// A Nutmeg progress model for running `cargo test` etc.
///
/// It draws the command and some description of what scenario is being tested.
struct CargoModel {
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
    fn new(scenario: &Scenario, start: Instant, log_file: Utf8PathBuf) -> CargoModel {
        let name: Cow<'static, str> = match scenario {
            Scenario::SourceTree => "Freshen source tree".into(),
            Scenario::Baseline => "Unmutated baseline".into(),
            Scenario::Mutant(mutant) => style_mutant(mutant).into(),
        };
        CargoModel {
            name,
            phase: None,
            start,
            log_file,
        }
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

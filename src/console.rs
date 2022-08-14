// Copyright 2021, 2022 Martin Pool

//! Print messages and progress bars on the terminal.

use std::borrow::Cow;
use std::fmt::Write;
use std::time::{Duration, Instant};

use ::console::{style, StyledObject};
use camino::Utf8Path;

use crate::outcome::SummaryOutcome;
use crate::*;

/// An interface to the console for the rest of cargo-mutants.
///
/// This wraps the Nutmeg view and model.
pub struct Console {
    /// The inner view through which progress bars and messages are drawn.
    view: nutmeg::View<LabModel>,
}

impl Console {
    pub fn new() -> Console {
        Console {
            view: nutmeg::View::new(LabModel::default(), nutmeg_options()),
        }
    }

    /// Update that cargo finished.
    pub fn cargo_outcome(
        &self,
        scenario: &Scenario,
        start: Instant,
        outcome: &Outcome,
        options: &Options,
    ) {
        self.view.update(|model| match outcome.summary() {
            SummaryOutcome::CaughtMutant => model.mutants_caught += 1,
            SummaryOutcome::MissedMutant => model.mutants_missed += 1,
            SummaryOutcome::Timeout => model.timeouts += 1,
            SummaryOutcome::Unviable => model.unviable += 1,
            SummaryOutcome::Success => model.successes += 1,
            SummaryOutcome::Failure => model.failures += 1,
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
            write!(s, " in {}", style_elapsed_secs(start)).unwrap();
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
        self.view.message(&s);
    }

    /// Update that work is starting on testing a given number of mutants.
    pub fn start_testing_mutants(&self, n_mutants: usize) {
        self.view.update(|model| {
            model.n_mutants = n_mutants;
            model.lab_start_time = Some(Instant::now());
        })
    }

    /// Update that a cargo task is starting.
    pub fn start_cargo(&self, scenario: &Scenario, log_file: &Utf8Path) {
        let start = Instant::now();
        let cargo_model = CargoModel::new(scenario, start, log_file.to_owned());
        self.view.update(|model| {
            model.i_mutant += scenario.is_mutant() as usize;
            model.cargo_model = Some(cargo_model);
        });
    }

    /// Update the phase of the cargo task.
    pub fn set_cargo_phase(&self, phase: &Phase) {
        self.view.update(|model| {
            model
                .cargo_model
                .as_mut()
                .expect("cargo model already started")
                .phase = Some(phase.name())
        })
    }

    pub fn message(&self, message: &str) {
        self.view.message(message)
    }

    pub fn tick(&self) {
        self.view.update(|_| ())
    }
}

/// Description of all current activities in the lab.
///
/// At the moment there is either a copy, cargo runs, or nothing.  Later, there
/// might be concurrent activities.
#[derive(Default)]
struct LabModel {
    copy_model: Option<CopyModel>,
    cargo_model: Option<CargoModel>,
    lab_start_time: Option<Instant>,
    i_mutant: usize,
    n_mutants: usize,
    mutants_caught: usize,
    mutants_missed: usize,
    unviable: usize,
    timeouts: usize,
    successes: usize,
    failures: usize,
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
                let elapsed = lab_start_time.elapsed();
                write!(
                    s,
                    "Trying mutant {}/{}, {} done",
                    self.i_mutant,
                    self.n_mutants,
                    nutmeg::percent_done(self.i_mutant, self.n_mutants),
                )
                .unwrap();
                if self.mutants_missed > 0 {
                    write!(s, ", {} missed", self.mutants_missed).unwrap();
                }
                if self.timeouts > 0 {
                    write!(s, ", {} timeouts", self.timeouts).unwrap();
                }
                if self.mutants_caught > 0 {
                    write!(s, ", {} caught", self.mutants_caught).unwrap();
                }
                if self.unviable > 0 {
                    write!(s, ", {} unviable", self.unviable).unwrap();
                }
                // Maybe don't report these, because they're uninteresting?
                // if self.successes > 0 {
                //     write!(s, ", {} successes", self.successes).unwrap();
                // }
                // if self.failures > 0 {
                //     write!(s, ", {} failures", self.failures).unwrap();
                // }
                writeln!(
                    s,
                    ", {} elapsed, about {} remaining",
                    duration_minutes_seconds(elapsed),
                    nutmeg::estimate_remaining(&lab_start_time, self.i_mutant, self.n_mutants)
                )
                .unwrap();
            }
            s.push_str(&cargo_model.render(width));
        }
        s
    }
}

impl LabModel {}

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
        write!(s, "... {}", style_elapsed_secs(self.start)).unwrap();
        if let Ok(last_line) = last_line(&self.log_file) {
            write!(s, "\n    {}", last_line).unwrap();
        }
        s
    }
}

impl CargoModel {
    fn new(scenario: &Scenario, start: Instant, log_file: Utf8PathBuf) -> CargoModel {
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
            style_elapsed_secs(self.start),
        )
    }

    fn final_message(&mut self) -> String {
        if self.succeeded {
            if self.show_times {
                format!(
                    "{} ... {} in {}",
                    self.name,
                    style_mb(self.bytes_copied),
                    style_elapsed_secs(self.start)
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

fn style_elapsed_secs(since: Instant) -> String {
    style(format!("{:.1}s", since.elapsed().as_secs_f32()))
        .cyan()
        .to_string()
}

pub fn duration_minutes_seconds(duration: Duration) -> String {
    let secs = duration.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
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

pub fn plural(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("{} {}", n, noun)
    } else {
        // TODO: Special cases for irregular nouns if they occur...
        format!("{} {}s", n, noun)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_duration_minutes_seconds() {
        assert_eq!(duration_minutes_seconds(Duration::ZERO), "0:00");
        assert_eq!(duration_minutes_seconds(Duration::from_secs(3)), "0:03");
        assert_eq!(duration_minutes_seconds(Duration::from_secs(73)), "1:13");
        assert_eq!(
            duration_minutes_seconds(Duration::from_secs(6003)),
            "100:03"
        );
    }
}

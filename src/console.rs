// Copyright 2021-2025 Martin Pool

//! Print messages and progress bars on the terminal.

use std::borrow::Cow;
use std::fmt::Write;
use std::fs::File;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use camino::{Utf8Path, Utf8PathBuf};
use console::{style, StyledObject};
use nutmeg::Destination;
use tracing::Level;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::prelude::*;

use crate::options::Colors;
use crate::outcome::{LabOutcome, ScenarioOutcome, SummaryOutcome};
use crate::scenario::Scenario;
use crate::tail_file::TailFile;
use crate::{Mutant, Options, Phase};

/// An interface to the console for the rest of cargo-mutants.
///
/// This wraps the Nutmeg view and model.
pub struct Console {
    /// The inner view through which progress bars and messages are drawn.
    view: Arc<nutmeg::View<LabModel>>,

    /// The `mutants.out/debug.log` file, if it's open yet.
    debug_log: Arc<Mutex<Option<File>>>,
}

impl Console {
    pub fn new() -> Console {
        Console {
            view: Arc::new(nutmeg::View::new(LabModel::default(), nutmeg_options())),
            debug_log: Arc::new(Mutex::new(None)),
        }
    }

    pub fn start_walk_tree(&self) -> WalkProgress {
        WalkProgress::new(&self.view)
    }

    /// Update that a cargo task is starting.
    pub fn scenario_started(&self, dir: &Utf8Path, scenario: &Scenario, log_file: File) {
        let start = Instant::now();
        let scenario_model = ScenarioModel::new(dir, scenario, start, log_file);
        self.view.update(|model| {
            model.scenario_models.push(scenario_model);
        });
    }

    /// Update that cargo finished.
    pub fn scenario_finished(
        &self,
        dir: &Utf8Path,
        scenario: &Scenario,
        outcome: &ScenarioOutcome,
        options: &Options,
    ) {
        self.view.update(|model| {
            if scenario.is_mutant() {
                model.mutants_done += 1;
            }
            match outcome.summary() {
                SummaryOutcome::CaughtMutant => model.mutants_caught += 1,
                SummaryOutcome::MissedMutant => model.mutants_missed += 1,
                SummaryOutcome::Timeout => model.timeouts += 1,
                SummaryOutcome::Unviable => model.unviable += 1,
                SummaryOutcome::Success => model.successes += 1,
                SummaryOutcome::Failure => model.failures += 1,
            }
            model.remove_scenario(dir);
        });

        if (outcome.mutant_caught() && !options.print_caught)
            || (outcome.scenario.is_mutant()
                && outcome.check_or_build_failed()
                && !options.print_unviable)
        {
            return;
        }

        let mut s = format!(
            "{:8} {}",
            style_outcome(outcome),
            style_scenario(scenario, true),
        );
        if options.show_times {
            let prs: Vec<String> = outcome
                .phase_results()
                .iter()
                .map(|pr| {
                    format!(
                        "{secs} {phase}",
                        secs = style_secs(pr.duration),
                        phase = style(pr.phase.to_string()).dim()
                    )
                })
                .collect();
            s.push_str(" in ");
            s.push_str(&prs.join(" + "));
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
        self.message(&s);
        if let Some(mutant) = scenario.mutant() {
            if outcome.mutant_missed() {
                let annotation = options.annotations.format(mutant);
                if !annotation.is_empty() {
                    self.message(&annotation);
                }
            }
        }
    }

    pub fn start_copy(&self, dir: &Utf8Path) {
        self.view.update(|model| {
            model.copy_models.push(CopyModel::new(dir.to_owned()));
        });
    }

    pub fn finish_copy(&self, dir: &Utf8Path) {
        self.view.update(|model| {
            let idx = model
                .copy_models
                .iter()
                .position(|m| m.dest == dir)
                .expect("copy model not found");
            model.copy_models.swap_remove(idx);
        });
    }

    pub fn copy_progress(&self, dest: &Utf8Path, total_bytes: u64) {
        self.view.update(|model| {
            model
                .copy_models
                .iter_mut()
                .find(|m| m.dest == dest)
                .expect("copy in progress")
                .bytes_copied(total_bytes);
        });
    }

    /// Update that we discovered some mutants to test.
    pub fn discovered_mutants(&self, mutants: &[Mutant]) {
        self.message(&format!(
            "Found {} to test\n",
            plural(mutants.len(), "mutant")
        ));
        let n_mutants = mutants.len();
        self.view.update(|model| {
            model.n_mutants = n_mutants;
            model.lab_start_time = Some(Instant::now());
        });
    }

    /// Update that work is starting on testing a given number of mutants.
    pub fn start_testing_mutants(&self, _n_mutants: usize) {
        self.view
            .update(|model| model.mutants_start_time = Some(Instant::now()));
    }

    /// A new phase of this scenario started.
    pub fn scenario_phase_started(&self, dir: &Utf8Path, phase: Phase) {
        self.view.update(|model| {
            model.find_scenario_mut(dir).phase_started(phase);
        });
    }

    pub fn scenario_phase_finished(&self, dir: &Utf8Path, phase: Phase) {
        self.view.update(|model| {
            model.find_scenario_mut(dir).phase_finished(phase);
        });
    }

    pub fn lab_finished(&self, lab_outcome: &LabOutcome, start_time: Instant, options: &Options) {
        self.view.update(|model| {
            model.scenario_models.clear();
        });
        self.message(&format!(
            "{}\n",
            lab_outcome.summary_string(start_time, options)
        ));
    }

    pub fn clear(&self) {
        self.view.clear();
    }

    pub fn message(&self, message: &str) {
        // A workaround for nutmeg not being able to coordinate writes to both stdout and
        // stderr...
        // <https://github.com/sourcefrog/nutmeg/issues/11>
        self.view.clear();
        print!("{message}");
    }

    pub fn tick(&self) {
        self.view.update(|_| ());
    }

    /// Return a tracing `MakeWriter` that will send messages via nutmeg to the console.
    pub fn make_terminal_writer(&self) -> TerminalWriter {
        TerminalWriter {
            view: Arc::clone(&self.view),
        }
    }

    /// Return a tracing `MakeWriter` that will send messages to the debug log file if
    /// it's open.
    pub fn make_debug_log_writer(&self) -> DebugLogWriter {
        DebugLogWriter(Arc::clone(&self.debug_log))
    }

    /// Set the debug log file.
    pub fn set_debug_log(&self, file: File) {
        *self.debug_log.lock().unwrap() = Some(file);
    }

    /// Configure tracing to send messages to the console and debug log.
    ///
    /// The debug log is opened later and provided by [`Console::set_debug_log`].
    pub fn setup_global_trace(&self, console_trace_level: Level, colors: Colors) {
        // Show time relative to the start of the program.
        let uptime = tracing_subscriber::fmt::time::uptime();
        let stderr_colors = colors
            .forced_value()
            .unwrap_or_else(::console::colors_enabled_stderr);
        let debug_log_layer = tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_file(true) // source file name
            .with_line_number(true)
            .with_timer(uptime)
            .with_writer(self.make_debug_log_writer());
        let level_filter = tracing_subscriber::filter::LevelFilter::from_level(console_trace_level);
        let console_layer = tracing_subscriber::fmt::layer()
            .with_ansi(stderr_colors)
            .with_writer(self.make_terminal_writer())
            .with_target(false)
            .without_time()
            .with_filter(level_filter);
        tracing_subscriber::registry()
            .with(debug_log_layer)
            .with(console_layer)
            .init();
    }
}

/// An interface for tree-walking code to publish progress.
pub(crate) struct WalkProgress {
    view: Arc<nutmeg::View<LabModel>>,
}

impl WalkProgress {
    fn new(view: &Arc<nutmeg::View<LabModel>>) -> WalkProgress {
        view.update(|model| {
            model.walk_tree = Some(WalkModel::default());
        });
        WalkProgress {
            view: Arc::clone(view),
        }
    }

    pub fn increment_files(&self, files_done: usize) {
        self.view.update(|model| {
            model.walk_tree.as_mut().unwrap().files_done += files_done;
        });
    }

    pub fn increment_mutants(&self, mutants_found: usize) {
        self.view.update(|model| {
            model.walk_tree.as_mut().unwrap().mutants_found += mutants_found;
        });
    }

    pub fn finish(self) {
        self.view.update(|model| model.walk_tree = None);
    }
}

pub fn enable_console_colors(colors: Colors) {
    if let Some(colors) = colors.forced_value() {
        ::console::set_colors_enabled(colors);
        ::console::set_colors_enabled_stderr(colors);
    }
    // Otherwise, let the console crate decide, based on isatty, etc.
}

impl Default for Console {
    fn default() -> Self {
        Self::new()
    }
}

/// Write trace output to the terminal via the console.
pub struct TerminalWriter {
    view: Arc<nutmeg::View<LabModel>>,
}

impl MakeWriter<'_> for TerminalWriter {
    type Writer = Self;

    fn make_writer(&self) -> Self::Writer {
        TerminalWriter {
            view: Arc::clone(&self.view),
        }
    }
}

impl std::io::Write for TerminalWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // This calls `message` rather than `View::write` because the latter
        // only requires a &View and it handles locking internally, without
        // requiring exclusive use of the Arc<View>.
        self.view.message(std::str::from_utf8(buf).unwrap());
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Write trace output to the debug log file if it's open.
pub struct DebugLogWriter(Arc<Mutex<Option<File>>>);

impl MakeWriter<'_> for DebugLogWriter {
    type Writer = Self;

    fn make_writer(&self) -> Self::Writer {
        DebugLogWriter(self.0.clone())
    }
}

impl io::Write for DebugLogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Some(file) = self.0.lock().unwrap().as_mut() {
            file.write(buf)
        } else {
            Ok(buf.len())
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(file) = self.0.lock().unwrap().as_mut() {
            file.flush()
        } else {
            Ok(())
        }
    }
}

/// Description of all current activities in the lab.
///
/// At the moment there is either a copy, cargo runs, or nothing.  Later, there
/// might be concurrent activities.
#[derive(Default)]
struct LabModel {
    walk_tree: Option<WalkModel>,
    /// Copy jobs in progress
    copy_models: Vec<CopyModel>,
    scenario_models: Vec<ScenarioModel>,
    lab_start_time: Option<Instant>,
    /// The instant when we started trying mutation scenarios, after running the baseline.
    mutants_start_time: Option<Instant>,
    mutants_done: usize,
    n_mutants: usize,
    mutants_caught: usize,
    mutants_missed: usize,
    unviable: usize,
    timeouts: usize,
    successes: usize,
    failures: usize,
}

impl nutmeg::Model for LabModel {
    #[allow(clippy::cast_precision_loss)]
    fn render(&mut self, width: usize) -> String {
        let mut s = String::with_capacity(1024);
        if let Some(walk_tree) = &mut self.walk_tree {
            s += &walk_tree.render(width);
        }
        for copy_model in &mut self.copy_models {
            if !s.is_empty() {
                s.push('\n');
            }
            s.push_str(&copy_model.render(width));
        }
        for sm in &mut self.scenario_models {
            if !s.is_empty() {
                s.push('\n');
            }
            s.push_str(&sm.render(width));
        }
        if let Some(lab_start_time) = self.lab_start_time {
            if !s.is_empty() {
                s.push('\n');
            }
            let elapsed = lab_start_time.elapsed();
            write!(
                s,
                "{}/{} mutants tested",
                style(self.mutants_done).cyan(),
                style(self.n_mutants).cyan(),
            )
            .unwrap();
            if self.mutants_missed > 0 {
                write!(
                    s,
                    ", {} {}",
                    style(self.mutants_missed).cyan(),
                    style("MISSED").red()
                )
                .unwrap();
            }
            if self.timeouts > 0 {
                write!(
                    s,
                    ", {} {}",
                    style(self.timeouts).cyan(),
                    style("timeout").red()
                )
                .unwrap();
            }
            if self.mutants_caught > 0 {
                write!(s, ", {} caught", style(self.mutants_caught).cyan()).unwrap();
            }
            if self.unviable > 0 {
                write!(s, ", {} unviable", style(self.unviable).cyan()).unwrap();
            }
            // Maybe don't report these, because they're uninteresting?
            // if self.successes > 0 {
            //     write!(s, ", {} successes", self.successes).unwrap();
            // }
            // if self.failures > 0 {
            //     write!(s, ", {} failures", self.failures).unwrap();
            // }
            write!(s, ", {} elapsed", style_duration(elapsed)).unwrap();
            if self.mutants_done > 2 {
                let done = self.mutants_done as f64;
                let remain = self.n_mutants as f64 - done;
                let mut remaining_secs =
                    self.mutants_start_time.unwrap().elapsed().as_secs_f64() * remain / done;
                if remaining_secs > 300.0 {
                    // Round up to minutes
                    remaining_secs = ((remaining_secs + 30.0) / 60.0).ceil() * 60.0;
                }
                write!(
                    s,
                    ", about {} remaining",
                    style_duration(Duration::from_secs_f64(remaining_secs.ceil()))
                )
                .unwrap();
            }
        }
        s
    }
}

impl LabModel {
    fn find_scenario_mut(&mut self, dir: &Utf8Path) -> &mut ScenarioModel {
        self.scenario_models
            .iter_mut()
            .find(|sm| sm.dir == *dir)
            .expect("scenario directory not found")
    }

    fn remove_scenario(&mut self, dir: &Utf8Path) {
        self.scenario_models.retain(|sm| sm.dir != *dir);
    }
}

/// A Nutmeg progress model for walking the tree.
#[derive(Default)]
struct WalkModel {
    files_done: usize,
    mutants_found: usize,
}

impl nutmeg::Model for WalkModel {
    fn render(&mut self, _width: usize) -> String {
        if self.files_done == 0 {
            "Scanning tree metadata...\n".to_owned()
        } else {
            format!(
                "Finding mutation opportunities: {} files done, {} mutants found\n",
                self.files_done, self.mutants_found
            )
        }
    }
}

/// A Nutmeg progress model for running a single scenario.
///
/// It draws the command and some description of what scenario is being tested.
struct ScenarioModel {
    /// The directory where this is being built: unique across all models.
    dir: Utf8PathBuf,
    name: Cow<'static, str>,
    phase_start: Instant,
    phase: Option<Phase>,
    /// Previously-executed phases and durations.
    previous_phase_durations: Vec<(Phase, Duration)>,
    log_tail: TailFile,
}

impl ScenarioModel {
    fn new(dir: &Utf8Path, scenario: &Scenario, start: Instant, log_file: File) -> ScenarioModel {
        ScenarioModel {
            dir: dir.to_owned(),
            name: style_scenario(scenario, true),
            phase: None,
            phase_start: start,
            log_tail: TailFile::new(log_file),
            previous_phase_durations: Vec::new(),
        }
    }

    fn phase_started(&mut self, phase: Phase) {
        self.phase = Some(phase);
        self.phase_start = Instant::now();
    }

    fn phase_finished(&mut self, phase: Phase) {
        debug_assert_eq!(self.phase, Some(phase));
        self.previous_phase_durations
            .push((phase, self.phase_start.elapsed()));
        self.phase = None;
    }
}

impl nutmeg::Model for ScenarioModel {
    fn render(&mut self, _width: usize) -> String {
        let mut parts = Vec::new();
        if let Some(phase) = self.phase {
            parts.push(style(format!("{phase:8}")).bold().cyan().to_string());
        }
        parts.push(self.name.to_string());
        parts.push("...".to_string());
        parts.push(style_secs(self.phase_start.elapsed()));
        // let mut prs = self
        //     .previous_phase_durations
        //     .iter()
        //     .map(|(phase, duration)| format!("{} {}", style_secs(*duration), style(phase).dim()))
        //     .collect::<Vec<_>>();
        // if prs.len() > 1 {
        //     prs.insert(0, String::new())
        // }
        // parts.push(prs.join(" + "));
        let mut s = parts.join(" ");
        if let Ok(last_line) = self.log_tail.last_line() {
            s.push('\n');
            s.push_str(&style("└       ").cyan().to_string());
            s.push(' ');
            s.push_str(style(last_line).dim().to_string().as_str());
        }
        s
    }
}

/// A Nutmeg model for progress in copying a tree.
struct CopyModel {
    dest: Utf8PathBuf,
    bytes_copied: u64,
    start: Instant,
}

impl CopyModel {
    fn new(dest: Utf8PathBuf) -> CopyModel {
        CopyModel {
            dest,
            start: Instant::now(),
            bytes_copied: 0,
        }
    }

    /// Update that some bytes have been copied.
    ///
    /// `bytes_copied` is the total bytes copied so far.
    fn bytes_copied(&mut self, bytes_copied: u64) {
        self.bytes_copied = bytes_copied;
    }
}

impl nutmeg::Model for CopyModel {
    fn render(&mut self, _width: usize) -> String {
        format!(
            "{:8} {} in {}",
            style("copy").cyan(),
            style_mb(self.bytes_copied),
            style_secs(self.start.elapsed()),
        )
    }
}

fn nutmeg_options() -> nutmeg::Options {
    nutmeg::Options::default()
        .update_interval(Duration::from_millis(250))
        .print_holdoff(Duration::from_millis(100))
        .destination(Destination::Stderr)
}

/// Return a styled string reflecting the moral value of this outcome.
pub fn style_outcome(outcome: &ScenarioOutcome) -> StyledObject<&'static str> {
    match outcome.summary() {
        SummaryOutcome::CaughtMutant => style("caught").green(),
        SummaryOutcome::MissedMutant => style("MISSED").red().bold(),
        SummaryOutcome::Failure => style("FAILED").red().bold(),
        SummaryOutcome::Success => style("ok").green(),
        SummaryOutcome::Unviable => style("unviable").blue(),
        SummaryOutcome::Timeout => style("TIMEOUT").red().bold(),
    }
}

fn style_secs(duration: Duration) -> String {
    style(format!("{:.1}s", duration.as_secs_f32()))
        .cyan()
        .to_string()
}

fn style_duration(duration: Duration) -> String {
    // We don't want silly precision.
    let duration = Duration::from_secs(duration.as_secs());
    style(format_duration(duration)).cyan().to_string()
}

/// Format a duration in a human-friendly way with only one component of precision.
pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs < 90 {
        format!("{secs}s")
    } else if secs < 90 * 60 {
        format!("{}m", (secs + 30) / 60)
    } else if secs < 36 * 3600 {
        format!("{}h", (secs + 1800) / 3600)
    } else {
        format!("{}d", (secs + 43200) / 86400)
    }
}

fn format_mb(bytes: u64) -> String {
    format!("{} MB", bytes / 1_000_000)
}

fn style_mb(bytes: u64) -> StyledObject<String> {
    style(format_mb(bytes)).cyan()
}

pub fn style_scenario(scenario: &Scenario, line_col: bool) -> Cow<'static, str> {
    match scenario {
        Scenario::Baseline => Cow::Borrowed("Unmutated baseline"),
        Scenario::Mutant(mutant) => Cow::Owned(mutant.to_styled_string(line_col)),
    }
}

pub fn plural(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("{n} {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn format_duration_test() {
        assert_eq!(format_duration(Duration::from_secs(0)), "0s");
        assert_eq!(format_duration(Duration::from_secs(1)), "1s");
        assert_eq!(format_duration(Duration::from_secs(10)), "10s");
        assert_eq!(format_duration(Duration::from_secs(89)), "89s");
        assert_eq!(format_duration(Duration::from_secs(90)), "2m");
        assert_eq!(format_duration(Duration::from_secs(119)), "2m");
        assert_eq!(format_duration(Duration::from_secs(120)), "2m");
        assert_eq!(format_duration(Duration::from_secs(3599)), "60m");
        assert_eq!(format_duration(Duration::from_secs(3600)), "60m");
        assert_eq!(format_duration(Duration::from_secs(7199)), "2h");
        assert_eq!(format_duration(Duration::from_secs(86399)), "24h");
        assert_eq!(format_duration(Duration::from_secs(86400)), "24h");
        assert_eq!(format_duration(Duration::from_secs(35 * 3600)), "35h");
        assert_eq!(format_duration(Duration::from_secs(2 * 86400)), "2d");
        assert_eq!(format_duration(Duration::from_secs(180 * 86400)), "180d");
    }
}

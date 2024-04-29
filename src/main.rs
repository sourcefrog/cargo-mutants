// Copyright 2021-2024 Martin Pool

//! `cargo-mutants`: Find test gaps by inserting bugs.
//!
//! See <https://mutants.rs> for more information.

mod build_dir;
mod cargo;
mod config;
mod console;
mod copy_tree;
mod exit_code;
mod fnvalue;
mod glob;
mod in_diff;
mod interrupt;
mod lab;
mod list;
mod log_file;
mod manifest;
mod mutate;
mod options;
mod outcome;
mod output;
mod package;
mod path;
mod pretty;
mod process;
mod scenario;
mod shard;
mod source;
mod span;
mod tail_file;
mod visit;
mod workspace;

use std::env;
use std::fs::read_to_string;
use std::io;
use std::process::exit;

use anyhow::{anyhow, ensure, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use clap::builder::styling::{self};
use clap::builder::Styles;
use clap::{ArgAction, CommandFactory, Parser, ValueEnum};
use clap_complete::{generate, Shell};
use color_print::cstr;
use tracing::debug;

use crate::build_dir::BuildDir;
use crate::console::Console;
use crate::in_diff::diff_filter;
use crate::interrupt::check_interrupted;
use crate::lab::test_mutants;
use crate::list::{list_files, list_mutants, FmtToIoWrite};
use crate::log_file::LogFile;
use crate::manifest::fix_manifest;
use crate::mutate::{Genre, Mutant};
use crate::options::{Colors, Options, TestTool};
use crate::outcome::{Phase, ScenarioOutcome};
use crate::scenario::Scenario;
use crate::shard::Shard;
use crate::workspace::{PackageFilter, Workspace};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const NAME: &str = env!("CARGO_PKG_NAME");

/// A comment marker inserted next to changes, so they can be easily found.
static MUTATION_MARKER_COMMENT: &str = "/* ~ changed by cargo-mutants ~ */";

static SPONSOR_MESSAGE: &str = cstr!("<magenta><bold>Support and accelerate cargo-mutants at <<https://github.com/sponsors/sourcefrog>></></>");

#[mutants::skip] // only visual effects, not worth testing
fn clap_styles() -> Styles {
    styling::Styles::styled()
        .header(styling::AnsiColor::Green.on_default() | styling::Effects::BOLD)
        .usage(styling::AnsiColor::Green.on_default() | styling::Effects::BOLD)
        .literal(styling::AnsiColor::Blue.on_default() | styling::Effects::BOLD)
        .placeholder(styling::AnsiColor::Cyan.on_default())
}

#[derive(Parser)]
#[command(name = "cargo", bin_name = "cargo", styles(clap_styles()))]
enum Cargo {
    #[command(name = "mutants", styles(clap_styles()))]
    Mutants(Args),
}

#[derive(Debug, Default, ValueEnum, Clone, Copy, Eq, PartialEq)]
pub enum BaselineStrategy {
    /// Run tests in an unmutated tree before testing mutants.
    #[default]
    Run,

    /// Don't run tests in an unmutated tree: assume that they pass.
    Skip,
}

/// Find inadequately-tested code that can be removed without any tests failing.
///
/// See <https://mutants.rs/> for more information.
#[derive(Parser, PartialEq, Debug)]
#[command(
    author,
    about,
    after_help = SPONSOR_MESSAGE,
)]
pub struct Args {
    /// Show cargo output for all invocations (very verbose).
    #[arg(long, help_heading = "Output")]
    all_logs: bool,

    /// Baseline strategy: check that tests pass in an unmutated tree before testing mutants.
    #[arg(long, value_enum, default_value_t = BaselineStrategy::Run, help_heading = "Execution")]
    baseline: BaselineStrategy,

    /// Print mutants that were caught by tests.
    #[arg(long, short = 'v', help_heading = "Output")]
    caught: bool,

    /// Cargo check generated mutants, but don't run tests.
    #[arg(long, help_heading = "Execution")]
    check: bool,

    /// Draw colors in output.
    #[arg(
        long,
        value_enum,
        help_heading = "Output",
        default_value_t,
        env = "CARGO_TERM_COLOR"
    )]
    colors: Colors,

    /// Show the mutation diffs.
    #[arg(long, help_heading = "Filters")]
    diff: bool,

    /// Rust crate directory to examine.
    #[arg(
        long,
        short = 'd',
        conflicts_with = "manifest_path",
        help_heading = "Input"
    )]
    dir: Option<Utf8PathBuf>,

    /// Generate autocompletions for the given shell.
    #[arg(long)]
    completions: Option<Shell>,

    /// Return this error values from functions returning Result:
    /// for example, `::anyhow::anyhow!("mutated")`.
    #[arg(long, help_heading = "Generate")]
    error: Vec<String>,

    /// Regex for mutations to examine, matched against the names shown by `--list`.
    #[arg(
        long = "re",
        short = 'F',
        alias = "regex",
        alias = "examine-regex",
        alias = "examine-re",
        help_heading = "Filters"
    )]
    examine_re: Vec<String>,

    /// Glob for files to exclude; with no glob, all files are included; globs containing
    /// slash match the entire path. If used together with `--file` argument, then the files to be examined are matched before the files to be excluded.
    #[arg(long, short = 'e', help_heading = "Filters")]
    exclude: Vec<String>,

    /// Regex for mutations to exclude, matched against the names shown by `--list`.
    #[arg(long, short = 'E', alias = "exclude-regex", help_heading = "Filters")]
    exclude_re: Vec<String>,

    /// Glob for files to examine; with no glob, all files are examined; globs containing
    /// slash match the entire path. If used together with `--exclude` argument, then the files to be examined are matched before the files to be excluded.
    #[arg(long, short = 'f', help_heading = "Filters")]
    file: Vec<String>,

    /// Don't copy files matching gitignore patterns.
    #[arg(long, action = ArgAction::Set, default_value = "true", help_heading = "Copying", group = "copy_opts")]
    gitignore: bool,

    /// Test mutations in the source tree, rather than in a copy.
    #[arg(
        long,
        help_heading = "Copying",
        conflicts_with = "jobs",
        conflicts_with = "copy_opts"
    )]
    in_place: bool,

    /// Run this many cargo build/test jobs in parallel.
    #[arg(
        long,
        short = 'j',
        env = "CARGO_MUTANTS_JOBS",
        help_heading = "Execution"
    )]
    jobs: Option<usize>,

    /// Output json (only for --list).
    #[arg(long, help_heading = "Output")]
    json: bool,

    /// Don't delete the scratch directories, for debugging.
    #[arg(long, help_heading = "Debug")]
    leak_dirs: bool,

    /// Log level for stdout (trace, debug, info, warn, error).
    #[arg(
        long,
        short = 'L',
        default_value = "info",
        env = "CARGO_MUTANTS_TRACE_LEVEL",
        help_heading = "Debug"
    )]
    level: tracing::Level,

    /// Just list possible mutants, don't run them.
    #[arg(long, help_heading = "Execution")]
    list: bool,

    /// List source files, don't run anything.
    #[arg(long, help_heading = "Execution")]
    list_files: bool,

    /// Path to Cargo.toml for the package to mutate.
    #[arg(long, help_heading = "Input")]
    manifest_path: Option<Utf8PathBuf>,

    /// Don't read .cargo/mutants.toml.
    #[arg(long, help_heading = "Input")]
    no_config: bool,

    /// Don't copy the /target directory, and don't build the source tree first.
    #[arg(long, help_heading = "Copying", group = "copy_opts")]
    no_copy_target: bool,

    /// Don't print times or tree sizes, to make output deterministic.
    #[arg(long, help_heading = "Output")]
    no_times: bool,

    /// Include line & column numbers in the mutation list.
    #[arg(long, action = ArgAction::Set, default_value = "true", help_heading = "Output")]
    line_col: bool,

    /// Create mutants.out within this directory.
    #[arg(long, short = 'o', help_heading = "Output")]
    output: Option<Utf8PathBuf>,

    /// Include only mutants in code touched by this diff.
    #[arg(long, short = 'D', help_heading = "Filters")]
    in_diff: Option<Utf8PathBuf>,

    /// Minimum timeout for tests, in seconds, as a lower bound on the auto-set time.
    #[arg(
        long,
        env = "CARGO_MUTANTS_MINIMUM_TEST_TIMEOUT",
        help_heading = "Execution"
    )]
    minimum_test_timeout: Option<f64>,

    /// Only test mutants from these packages.
    #[arg(id = "package", long, short = 'p', help_heading = "Filters")]
    mutate_packages: Vec<String>,

    /// Run mutants in random order.
    #[arg(long, help_heading = "Execution")]
    shuffle: bool,

    /// Run mutants in the fixed order they occur in the source tree.
    #[arg(long, help_heading = "Execution")]
    no_shuffle: bool,

    /// Run only one shard of all generated mutants: specify as e.g. 1/4.
    #[arg(long, help_heading = "Execution")]
    shard: Option<Shard>,

    /// Tool used to run test suites: cargo or nextest.
    #[arg(long, help_heading = "Execution")]
    test_tool: Option<TestTool>,

    /// Maximum run time for all cargo commands, in seconds.
    #[arg(long, short = 't', help_heading = "Execution")]
    timeout: Option<f64>,

    /// Test timeout multiplier (relative to base test time).
    #[arg(long, help_heading = "Execution", conflicts_with = "timeout")]
    timeout_multiplier: Option<f64>,

    /// Maximum run time for cargo build command, in seconds.
    #[arg(long, help_heading = "Execution")]
    build_timeout: Option<f64>,

    /// Build timeout multiplier (relative to base build time).
    #[arg(long, help_heading = "Execution", conflicts_with = "build_timeout")]
    build_timeout_multiplier: Option<f64>,

    /// Print mutations that failed to check or build.
    #[arg(long, short = 'V', help_heading = "Output")]
    unviable: bool,

    /// Show version and quit.
    #[arg(long, action = clap::ArgAction::SetTrue)]
    version: bool,

    /// Test every package in the workspace.
    #[arg(long, help_heading = "Filters")]
    workspace: bool,

    /// Additional args for all cargo invocations.
    #[arg(
        long,
        short = 'C',
        allow_hyphen_values = true,
        help_heading = "Execution"
    )]
    cargo_arg: Vec<String>,

    /// Pass remaining arguments to cargo test after all options and after `--`.
    #[arg(last = true, help_heading = "Execution")]
    cargo_test_args: Vec<String>,

    #[command(flatten)]
    features: Features,
}

#[derive(clap::Args, PartialEq, Eq, Debug, Default, Clone)]
pub struct Features {
    //---  features
    /// Space or comma separated list of features to activate.
    // (The features are not split or parsed, just passed through to Cargo.)
    #[arg(long, help_heading = "Feature Selection")]
    pub features: Vec<String>,

    /// Do not activate the `default` feature.
    #[arg(long, help_heading = "Feature Selection")]
    pub no_default_features: bool,

    /// Activate all features.
    // (This does not conflict because this only turns on features in the top level package,
    // and you might use --features to turn on features in dependencies.)
    #[arg(long, help_heading = "Feature Selection")]
    pub all_features: bool,
}

fn main() -> Result<()> {
    let args = match Cargo::try_parse() {
        Ok(Cargo::Mutants(args)) => args,
        Err(e) => {
            e.print().expect("Failed to show clap error message");
            // Clap by default exits with code 2.
            let code = match e.exit_code() {
                2 => exit_code::USAGE,
                0 => 0,
                _ => exit_code::SOFTWARE,
            };
            exit(code);
        }
    };

    if args.version {
        println!("{NAME} {VERSION}");
        return Ok(());
    } else if let Some(shell) = args.completions {
        generate(shell, &mut Cargo::command(), "cargo", &mut io::stdout());
        return Ok(());
    }

    let console = Console::new();
    console.setup_global_trace(args.level, args.colors)?; // We don't have Options yet.
    console.set_colors_enabled(args.colors);
    interrupt::install_handler();

    let start_dir: &Utf8Path = if let Some(manifest_path) = &args.manifest_path {
        ensure!(manifest_path.is_file(), "Manifest path is not a file");
        manifest_path
            .parent()
            .ok_or(anyhow!("Manifest path has no parent"))?
    } else if let Some(dir) = &args.dir {
        dir
    } else {
        Utf8Path::new(".")
    };
    let workspace = Workspace::open(start_dir)?;
    let config = if args.no_config {
        config::Config::default()
    } else {
        config::Config::read_tree_config(&workspace.dir)?
    };
    debug!(?config);
    debug!(?args.features);
    let options = Options::new(&args, &config)?;
    debug!(?options);
    let package_filter = if !args.mutate_packages.is_empty() {
        PackageFilter::explicit(&args.mutate_packages)
    } else if args.workspace {
        PackageFilter::All
    } else {
        PackageFilter::Auto(start_dir.to_owned())
    };
    let discovered = workspace.discover(&package_filter, &options, &console)?;
    console.clear();
    if args.list_files {
        list_files(FmtToIoWrite::new(io::stdout()), &discovered.files, &options)?;
        return Ok(());
    }
    let mut mutants = discovered.mutants;
    if let Some(in_diff) = &args.in_diff {
        mutants = diff_filter(
            mutants,
            &read_to_string(in_diff).context("Failed to read filter diff")?,
        )?;
    }
    if let Some(shard) = &args.shard {
        mutants = shard.select(mutants);
    }
    if args.list {
        list_mutants(FmtToIoWrite::new(io::stdout()), &mutants, &options)?;
    } else {
        let lab_outcome = test_mutants(mutants, &workspace.dir, options, &console)?;
        exit(lab_outcome.exit_code());
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use clap::CommandFactory;

    #[test]
    fn option_help_sentence_case_without_period() {
        let args = super::Args::command();
        let mut problems = Vec::new();
        for arg in args.get_arguments() {
            if let Some(help) = arg.get_help().map(|s| s.to_string()) {
                if !help.starts_with(char::is_uppercase) {
                    problems.push(format!(
                        "Help for {:?} does not start with a capital letter: {:?}",
                        arg.get_id(),
                        help
                    ));
                }
                // Clap seems to automatically strip periods from the end of help text in docstrings,
                // but let's leave this here just in case.
                if help.ends_with('.') {
                    problems.push(format!(
                        "Help for {:?} ends with a period: {:?}",
                        arg.get_id(),
                        help
                    ));
                }
                if help.is_empty() {
                    problems.push(format!("Help for {:?} is empty", arg.get_id()));
                }
            } else {
                problems.push(format!("No help for {:?}", arg.get_id()));
            }
        }
        problems.iter().for_each(|s| eprintln!("{s}"));
        if !problems.is_empty() {
            panic!("Problems with help text");
        }
    }
}

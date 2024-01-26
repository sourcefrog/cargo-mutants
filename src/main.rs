// Copyright 2021-2024 Martin Pool

//! `cargo-mutants`: Find test gaps by inserting bugs.

mod build_dir;
mod cargo;
mod config;
mod console;
mod copy_tree;
mod exit_code;
mod fnvalue;
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
/// See <https://github.com/sourcefrog/cargo-mutants> for more information.
#[derive(Parser, PartialEq, Debug)]
#[command(
    author,
    about,
    after_help = SPONSOR_MESSAGE,
)]
struct Args {
    /// show cargo output for all invocations (very verbose).
    #[arg(long, help_heading = "Output")]
    all_logs: bool,

    /// baseline strategy: check that tests pass in an unmutated tree before testing mutants.
    #[arg(long, value_enum, default_value_t = BaselineStrategy::Run, help_heading = "Execution")]
    baseline: BaselineStrategy,

    /// print mutants that were caught by tests.
    #[arg(long, short = 'v', help_heading = "Output")]
    caught: bool,

    /// cargo check generated mutants, but don't run tests.
    #[arg(long, help_heading = "Execution")]
    check: bool,

    /// draw colors in output.
    #[arg(
        long,
        value_enum,
        help_heading = "Output",
        default_value_t,
        env = "CARGO_TERM_COLOR"
    )]
    colors: Colors,

    /// show the mutation diffs.
    #[arg(long, help_heading = "Filters")]
    diff: bool,

    /// rust crate directory to examine.
    #[arg(
        long,
        short = 'd',
        conflicts_with = "manifest_path",
        help_heading = "Input"
    )]
    dir: Option<Utf8PathBuf>,

    /// generate autocompletions for the given shell.
    #[arg(long)]
    completions: Option<Shell>,

    /// return this error values from functions returning Result:
    /// for example, `::anyhow::anyhow!("mutated")`.
    #[arg(long, help_heading = "Generate")]
    error: Vec<String>,

    /// regex for mutations to examine, matched against the names shown by `--list`.
    #[arg(
        long = "re",
        short = 'F',
        alias = "regex",
        alias = "examine-regex",
        alias = "examine-re",
        help_heading = "Filters"
    )]
    examine_re: Vec<String>,

    /// glob for files to exclude; with no glob, all files are included; globs containing
    /// slash match the entire path. If used together with `--file` argument, then the files to be examined are matched before the files to be excluded.
    #[arg(long, short = 'e', help_heading = "Filters")]
    exclude: Vec<String>,

    /// regex for mutations to exclude, matched against the names shown by `--list`.
    #[arg(long, short = 'E', alias = "exclude-regex", help_heading = "Filters")]
    exclude_re: Vec<String>,

    /// glob for files to examine; with no glob, all files are examined; globs containing
    /// slash match the entire path. If used together with `--exclude` argument, then the files to be examined are matched before the files to be excluded.
    #[arg(long, short = 'f', help_heading = "Filters")]
    file: Vec<String>,

    /// don't copy files matching gitignore patterns.
    #[arg(long, action = ArgAction::Set, default_value = "true", help_heading = "Copying", group = "copy_opts")]
    gitignore: bool,

    /// test mutations in the source tree, rather than in a copy.
    #[arg(
        long,
        help_heading = "Copying",
        conflicts_with = "jobs",
        conflicts_with = "copy_opts"
    )]
    in_place: bool,

    /// run this many cargo build/test jobs in parallel.
    #[arg(
        long,
        short = 'j',
        env = "CARGO_MUTANTS_JOBS",
        help_heading = "Execution"
    )]
    jobs: Option<usize>,

    /// output json (only for --list).
    #[arg(long, help_heading = "Output")]
    json: bool,

    /// don't delete the scratch directories, for debugging.
    #[arg(long, help_heading = "Debug")]
    leak_dirs: bool,

    /// log level for stdout (trace, debug, info, warn, error).
    #[arg(
        long,
        short = 'L',
        default_value = "info",
        env = "CARGO_MUTANTS_TRACE_LEVEL",
        help_heading = "Debug"
    )]
    level: tracing::Level,

    /// just list possible mutants, don't run them.
    #[arg(long, help_heading = "Execution")]
    list: bool,

    /// list source files, don't run anything.
    #[arg(long, help_heading = "Execution")]
    list_files: bool,

    /// path to Cargo.toml for the package to mutate.
    #[arg(long, help_heading = "Input")]
    manifest_path: Option<Utf8PathBuf>,

    /// don't read .cargo/mutants.toml.
    #[arg(long, help_heading = "Input")]
    no_config: bool,

    /// don't copy the /target directory, and don't build the source tree first.
    #[arg(long, help_heading = "Copying", group = "copy_opts")]
    no_copy_target: bool,

    /// don't print times or tree sizes, to make output deterministic.
    #[arg(long, help_heading = "Output")]
    no_times: bool,

    /// include line & column numbers in the mutation list.
    #[arg(long, action = ArgAction::Set, default_value = "true", help_heading = "Output")]
    line_col: bool,

    /// create mutants.out within this directory.
    #[arg(long, short = 'o', help_heading = "Output")]
    output: Option<Utf8PathBuf>,

    /// include only mutants in code touched by this diff.
    #[arg(long, short = 'D', help_heading = "Filters")]
    in_diff: Option<Utf8PathBuf>,

    /// minimum timeout for tests, in seconds, as a lower bound on the auto-set time.
    #[arg(
        long,
        env = "CARGO_MUTANTS_MINIMUM_TEST_TIMEOUT",
        help_heading = "Execution"
    )]
    minimum_test_timeout: Option<f64>,

    /// only test mutants from these packages.
    #[arg(id = "package", long, short = 'p', help_heading = "Filters")]
    mutate_packages: Vec<String>,

    /// run mutants in random order.
    #[arg(long, help_heading = "Execution")]
    shuffle: bool,

    /// run mutants in the fixed order they occur in the source tree.
    #[arg(long, help_heading = "Execution")]
    no_shuffle: bool,

    /// run only one shard of all generated mutants: specify as e.g. 1/4.
    #[arg(long, help_heading = "Execution")]
    shard: Option<Shard>,

    /// tool used to run test suites: cargo or nextest.
    #[arg(long, help_heading = "Execution")]
    test_tool: Option<TestTool>,

    /// maximum run time for all cargo commands, in seconds.
    #[arg(long, short = 't', help_heading = "Execution")]
    timeout: Option<f64>,

    /// print mutations that failed to check or build.
    #[arg(long, short = 'V', help_heading = "Output")]
    unviable: bool,

    /// show version and quit.
    #[arg(long, action = clap::ArgAction::SetTrue)]
    version: bool,

    /// test every package in the workspace.
    #[arg(long, help_heading = "Filters")]
    workspace: bool,

    /// additional args for all cargo invocations.
    #[arg(
        long,
        short = 'C',
        allow_hyphen_values = true,
        help_heading = "Execution"
    )]
    cargo_arg: Vec<String>,

    // The following option captures all the remaining non-option args, to
    // send to cargo.
    /// pass remaining arguments to cargo test after all options and after `--`.
    #[arg(last = true, help_heading = "Execution")]
    cargo_test_args: Vec<String>,
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

// Copyright 2021-2023 Martin Pool

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
use clap::{ArgAction, CommandFactory, Parser};
use clap_complete::{generate, Shell};
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
use crate::options::Options;
use crate::outcome::{Phase, ScenarioOutcome};
use crate::scenario::Scenario;
use crate::workspace::{PackageFilter, Workspace};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const NAME: &str = env!("CARGO_PKG_NAME");

/// A comment marker inserted next to changes, so they can be easily found.
static MUTATION_MARKER_COMMENT: &str = "/* ~ changed by cargo-mutants ~ */";

#[derive(Parser)]
#[command(name = "cargo", bin_name = "cargo")]
enum Cargo {
    #[command(name = "mutants")]
    Mutants(Args),
}

/// Find inadequately-tested code that can be removed without any tests failing.
///
/// See <https://github.com/sourcefrog/cargo-mutants> for more information.
#[derive(Parser, PartialEq, Debug)]
#[command(author, about)]
struct Args {
    /// show cargo output for all invocations (very verbose).
    #[arg(long)]
    all_logs: bool,

    /// print mutants that were caught by tests.
    #[arg(long, short = 'v')]
    caught: bool,

    /// cargo check generated mutants, but don't run tests.
    #[arg(long)]
    check: bool,

    /// generate autocompletions for the given shell.
    #[arg(long)]
    completions: Option<Shell>,

    /// show the mutation diffs.
    #[arg(long)]
    diff: bool,

    /// rust crate directory to examine.
    #[arg(long, short = 'd', conflicts_with = "manifest_path")]
    dir: Option<Utf8PathBuf>,

    /// return this error values from functions returning Result:
    /// for example, `::anyhow::anyhow!("mutated")`.
    #[arg(long)]
    error: Vec<String>,

    /// regex for mutations to examine, matched against the names shown by `--list`.
    #[arg(
        long = "re",
        short = 'F',
        alias = "regex",
        alias = "examine-regex",
        alias = "examine-re"
    )]
    examine_re: Vec<String>,

    /// glob for files to exclude; with no glob, all files are included; globs containing
    /// slash match the entire path. If used together with `--file` argument, then the files to be examined are matched before the files to be excluded.
    #[arg(long, short = 'e')]
    exclude: Vec<String>,

    /// regex for mutations to exclude, matched against the names shown by `--list`.
    #[arg(long, short = 'E', alias = "exclude-regex")]
    exclude_re: Vec<String>,

    /// glob for files to examine; with no glob, all files are examined; globs containing
    /// slash match the entire path. If used together with `--exclude` argument, then the files to be examined are matched before the files to be excluded.
    #[arg(long, short = 'f')]
    file: Vec<String>,

    /// don't copy files matching gitignore patterns.
    #[arg(long, action = ArgAction::Set, default_value = "true")]
    gitignore: bool,

    /// run this many cargo build/test jobs in parallel.
    #[arg(long, short = 'j', env = "CARGO_MUTANTS_JOBS")]
    jobs: Option<usize>,

    /// output json (only for --list).
    #[arg(long)]
    json: bool,

    /// don't delete the scratch directories, for debugging.
    #[arg(long)]
    leak_dirs: bool,

    /// log level for stdout (trace, debug, info, warn, error).
    #[arg(
        long,
        short = 'L',
        default_value = "info",
        env = "CARGO_MUTANTS_TRACE_LEVEL"
    )]
    level: tracing::Level,

    /// just list possible mutants, don't run them.
    #[arg(long)]
    list: bool,

    /// list source files, don't run anything.
    #[arg(long)]
    list_files: bool,

    /// path to Cargo.toml for the package to mutate.
    #[arg(long)]
    manifest_path: Option<Utf8PathBuf>,

    /// don't read .cargo/mutants.toml.
    #[arg(long)]
    no_config: bool,

    /// don't copy the /target directory, and don't build the source tree first.
    #[arg(long)]
    no_copy_target: bool,

    /// don't print times or tree sizes, to make output deterministic.
    #[arg(long)]
    no_times: bool,

    /// include line & column numbers in the mutation list.
    #[arg(long, action = ArgAction::Set, default_value = "true")]
    line_col: bool,

    /// create mutants.out within this directory.
    #[arg(long, short = 'o')]
    output: Option<Utf8PathBuf>,

    /// include only mutants in code touched by this diff.
    #[arg(long, short = 'D')]
    in_diff: Option<Utf8PathBuf>,

    /// only test mutants from these packages.
    #[arg(id = "package", long, short = 'p')]
    mutate_packages: Vec<String>,

    /// run mutants in random order.
    #[arg(long)]
    shuffle: bool,

    /// run mutants in the fixed order they occur in the source tree.
    #[arg(long)]
    no_shuffle: bool,

    /// maximum run time for all cargo commands, in seconds.
    #[arg(long, short = 't')]
    timeout: Option<f64>,

    /// minimum timeout for tests, in seconds, as a lower bound on the auto-set time.
    #[arg(long, env = "CARGO_MUTANTS_MINIMUM_TEST_TIMEOUT")]
    minimum_test_timeout: Option<f64>,

    /// print mutations that failed to check or build.
    #[arg(long, short = 'V')]
    unviable: bool,

    /// show version and quit.
    #[arg(long, action = clap::ArgAction::SetTrue)]
    version: bool,

    /// test every package in the workspace.
    #[arg(long)]
    workspace: bool,

    /// additional args for all cargo invocations.
    #[arg(long, short = 'C', allow_hyphen_values = true)]
    cargo_arg: Vec<String>,

    // The following option captures all the remaining non-option args, to
    // send to cargo.
    /// pass remaining arguments to cargo test after all options and after `--`.
    #[arg(last = true)]
    cargo_test_args: Vec<String>,
}

fn main() -> Result<()> {
    let args = match Cargo::try_parse() {
        Ok(Cargo::Mutants(args)) => args,
        Err(e) => {
            eprintln!("{e}");
            exit(exit_code::USAGE);
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
    console.setup_global_trace(args.level)?;
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
    if args.list {
        list_mutants(FmtToIoWrite::new(io::stdout()), &mutants, &options)?;
    } else {
        let lab_outcome = test_mutants(mutants, &workspace.dir, options, &console)?;
        exit(lab_outcome.exit_code());
    }
    Ok(())
}

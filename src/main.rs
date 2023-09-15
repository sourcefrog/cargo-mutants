// Copyright 2021-2023 Martin Pool

//! `cargo-mutants`: Find inadequately-tested code that can be removed without any tests failing.

mod build_dir;
mod cargo;
mod config;
mod console;
mod exit_code;
mod fnvalue;
mod interrupt;
mod lab;
mod list;
mod log_file;
mod manifest;
mod mutate;
mod options;
mod outcome;
mod output;
mod path;
mod pretty;
mod process;
mod scenario;
mod source;
mod textedit;
mod tool;
mod visit;

use std::env;
use std::io;
use std::process::exit;

use anyhow::Result;
use camino::Utf8Path;
use camino::Utf8PathBuf;
use clap::CommandFactory;
use clap::Parser;
use clap_complete::{generate, Shell};
use path_slash::PathExt;
use tracing::debug;

// Imports of public names from this crate.
use crate::build_dir::BuildDir;
use crate::cargo::CargoTool;
use crate::console::Console;
use crate::interrupt::check_interrupted;
use crate::list::{list_files, list_mutants, GlueWrite};
use crate::log_file::{last_line, LogFile};
use crate::manifest::fix_manifest;
use crate::mutate::{Genre, Mutant};
use crate::options::Options;
use crate::outcome::{Phase, ScenarioOutcome};
use crate::path::Utf8PathSlashes;
use crate::scenario::Scenario;
use crate::source::SourceFile;
use crate::tool::Tool;
use crate::visit::walk_tree;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const NAME: &str = env!("CARGO_PKG_NAME");

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
    #[arg(long, short = 'd')]
    dir: Option<Utf8PathBuf>,

    /// return this error values from functions returning Result:
    /// for example, `::anyhow::anyhow!("mutated")`.
    #[arg(long)]
    error: Vec<String>,

    /// regex for mutations to examine, matched against the names shown by `--list`.
    #[arg(long = "re", short = 'F')]
    examine_re: Vec<String>,

    /// glob for files to exclude; with no glob, all files are included; globs containing
    /// slash match the entire path. If used together with `--file` argument, then the files to be examined are matched before the files to be excluded.
    #[arg(long, short = 'e')]
    exclude: Vec<String>,

    /// regex for mutations to exclude, matched against the names shown by `--list`.
    #[arg(long, short = 'E')]
    exclude_re: Vec<String>,

    /// glob for files to examine; with no glob, all files are examined; globs containing
    /// slash match the entire path. If used together with `--exclude` argument, then the files to be examined are matched before the files to be excluded.
    #[arg(long, short = 'f')]
    file: Vec<String>,

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

    /// don't read .cargo/mutants.toml.
    #[arg(long)]
    no_config: bool,

    /// don't copy the /target directory, and don't build the source tree first.
    #[arg(long)]
    no_copy_target: bool,

    /// don't print times or tree sizes, to make output deterministic.
    #[arg(long)]
    no_times: bool,

    /// create mutants.out within this directory.
    #[arg(long, short = 'o')]
    output: Option<Utf8PathBuf>,

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
    let console = Console::new();
    console.setup_global_trace(args.level)?;
    interrupt::install_handler();

    if args.version {
        println!("{NAME} {VERSION}");
        return Ok(());
    } else if let Some(shell) = args.completions {
        generate(shell, &mut Cargo::command(), "cargo", &mut io::stdout());
        return Ok(());
    }

    let source_path: &Utf8Path = if let Some(p) = &args.dir {
        p
    } else {
        Utf8Path::new(".")
    };
    let tool = CargoTool::new();
    let source_tree_root = tool.find_root(source_path)?;
    let config;
    if args.no_config {
        config = config::Config::default();
    } else {
        config = config::Config::read_tree_config(&source_tree_root)?;
        debug!(?config);
    }
    let options = Options::new(&args, &config)?;
    debug!(?options);
    if args.list_files {
        list_files(
            GlueWrite::new(io::stdout()),
            &tool,
            &source_tree_root,
            &options,
        )?;
    } else if args.list {
        list_mutants(
            GlueWrite::new(io::stdout()),
            &tool,
            &source_tree_root,
            &options,
        )?;
    } else {
        let lab_outcome =
            lab::test_unmutated_then_all_mutants(&tool, &source_tree_root, options, &console)?;
        exit(lab_outcome.exit_code());
    }
    Ok(())
}

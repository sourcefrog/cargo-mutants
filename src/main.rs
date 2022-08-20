// Copyright 2021, 2022 Martin Pool

//! `cargo-mutants`: Find inadequately-tested code that can be removed without any tests failing.

mod build_dir;
mod cargo;
mod console;
mod exit_code;
mod interrupt;
mod lab;
mod log_file;
mod manifest;
mod mutate;
mod options;
mod outcome;
mod output;
mod path;
mod source;
mod textedit;
mod visit;

use std::convert::TryFrom;
use std::env;
use std::io;
use std::process::exit;
use std::time::Duration;

use anyhow::Result;
use argh::FromArgs;
use camino::Utf8PathBuf;
use path_slash::PathExt;

// Imports of public names from this crate.
use crate::build_dir::BuildDir;
use crate::cargo::CargoResult;
use crate::interrupt::check_interrupted;
use crate::lab::Scenario;
use crate::log_file::{last_line, LogFile};
use crate::manifest::fix_manifest;
use crate::mutate::{Mutant, MutationOp};
use crate::options::Options;
use crate::outcome::{Outcome, Phase};
use crate::path::Utf8PathSlashes;
use crate::source::{SourceFile, SourceTree};
use crate::visit::discover_mutants;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const NAME: &str = env!("CARGO_PKG_NAME");
const DEFAULT_MINIMUM_TEST_TIMEOUT: Duration = Duration::from_secs(20);
const MINIMUM_TEST_TIMEOUT_ENV_VAR: &str = "CARGO_MUTANTS_MINIMUM_TEST_TIMEOUT";

/// Find inadequately-tested code that can be removed without any tests failing.
///
/// See <https://github.com/sourcefrog/cargo-mutants> for more information.
#[derive(FromArgs, PartialEq, Debug)]
struct Args {
    /// show cargo output for all invocations (very verbose).
    #[argh(switch)]
    all_logs: bool,

    /// print mutants that were caught by tests.
    #[argh(switch, short = 'v')]
    caught: bool,

    /// cargo check generated mutants, but don't run tests.
    #[argh(switch)]
    check: bool,

    /// show the mutation diffs.
    #[argh(switch)]
    diff: bool,

    /// rust crate directory to examine.
    #[argh(option, short = 'd', default = r#"Utf8PathBuf::from(".")"#)]
    dir: Utf8PathBuf,

    /// glob for files to examine; with no glob, all files are examined; globs containing
    /// slash match the entire path. If used together with `--exclude` argument, then the files to be examined are matched before the files to be excluded.
    #[argh(option, short = 'f')]
    file: Vec<String>,

    /// glob for files to exclude; with no glob, all files are included; globs containing
    /// slash match the entire path. If used together with `--file` argument, then the files to be examined are matched before the files to be excluded.
    #[argh(option, short = 'e')]
    exclude: Vec<String>,

    /// output json (only for --list).
    #[argh(switch)]
    json: bool,

    /// just list possible mutants, don't run them.
    #[argh(switch)]
    list: bool,

    /// list source files, don't run anything.
    #[argh(switch)]
    list_files: bool,

    /// don't copy the /target directory, and don't build the source tree first.
    #[argh(switch)]
    no_copy_target: bool,

    /// don't print times or tree sizes, to make output deterministic.
    #[argh(switch)]
    no_times: bool,

    /// create mutants.out within this directory.
    #[argh(option, short = 'o')]
    output: Option<Utf8PathBuf>,

    /// run mutants in random order.
    #[argh(switch)]
    shuffle: bool,

    /// run mutants in the fixed order they occur in the source tree.
    #[argh(switch)]
    no_shuffle: bool,

    /// maximum run time for all cargo commands, in seconds.
    #[argh(option, short = 't')]
    timeout: Option<f64>,

    /// print mutations that failed to check or build.
    #[argh(switch, short = 'V')]
    unviable: bool,

    /// show version and quit.
    #[argh(switch)]
    version: bool,

    /// additional args for all cargo invocations.
    #[argh(option, short = 'C')]
    cargo_arg: Vec<String>,

    // The following option captures all the remaining non-option args, to
    // send to cargo.
    /// pass remaining arguments to cargo test after all options and after `--`.
    #[argh(positional)]
    cargo_test_args: Vec<String>,
}

fn main() -> Result<()> {
    if let Some(subcommand) = env::args().nth(1) {
        if subcommand != "mutants" {
            eprintln!("unrecognized cargo subcommand {:?}", subcommand);
            exit(exit_code::USAGE);
        }
    } else {
        eprintln!("usage: cargo mutants <ARGS>\n   or: cargo-mutants mutants <ARGS>");
        exit(exit_code::USAGE);
    }
    let args: Args = argh::cargo_from_env();
    let options = Options::try_from(&args)?;
    let source_tree = SourceTree::new(&args.dir)?;
    interrupt::install_handler();
    if args.version {
        println!("{} {}", NAME, VERSION);
    } else if args.list_files {
        let files: Vec<String> = source_tree
            .source_paths(&options)?
            .into_iter()
            .map(|trp| trp.to_string())
            .collect();
        if args.json {
            serde_json::to_writer_pretty(io::BufWriter::new(io::stdout()), &files)?;
        } else {
            for f in files {
                println!("{}", f);
            }
        }
    } else if args.list {
        let mutants = source_tree.mutants(&options)?;
        if args.json {
            if args.diff {
                eprintln!("--list --diff --json is not (yet) supported");
                exit(exit_code::USAGE);
            }
            serde_json::to_writer_pretty(io::BufWriter::new(io::stdout()), &mutants)?;
        } else {
            console::list_mutants(&mutants, args.diff);
        }
    } else {
        let lab_outcome = lab::test_unmutated_then_all_mutants(&source_tree, &options)?;
        exit(lab_outcome.exit_code());
    }
    Ok(())
}

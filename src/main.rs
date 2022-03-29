// Copyright 2021, 2022 Martin Pool

//! `cargo-mutants`: Find inadequately-tested code that can be removed without any tests failing.

mod console;
mod exit_code;
mod interrupt;
mod lab;
mod log_file;
mod mutate;
mod options;
mod outcome;
mod output;
mod run;
mod source;
mod textedit;
mod visit;

use std::env;
use std::io;
use std::path::PathBuf;
use std::process::exit;

use anyhow::Result;
use argh::FromArgs;
#[allow(unused)]
use path_slash::PathExt;

// Imports of public names from this crate.
use crate::interrupt::check_interrupted;
use crate::lab::Scenario;
use crate::log_file::LogFile;
use crate::mutate::{Mutant, MutationOp};
use crate::options::Options;
use crate::outcome::{Outcome, Phase};
use crate::run::CargoResult;
use crate::source::{SourceFile, SourceTree};
use crate::visit::discover_mutants;

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
    #[argh(option, short = 'd', default = r#"PathBuf::from(".")"#)]
    dir: PathBuf,

    /// output json (only for --list).
    #[argh(switch)]
    json: bool,

    /// just list possible mutants, don't run them.
    #[argh(switch)]
    list: bool,

    /// don't copy the /target directory, and don't build the source tree first.
    #[argh(switch)]
    no_copy_target: bool,

    /// don't print times or tree sizes, to make output deterministic.
    #[argh(switch)]
    no_times: bool,

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
    let source_tree = SourceTree::new(&args.dir)?;
    let options = Options::from(&args);
    interrupt::install_handler();
    if args.list {
        let mutants = source_tree.mutants()?;
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
        // TODO: Perhaps print a text summary of how many were tested and whether they were all
        // caught?
        exit(lab_outcome.exit_code());
    }
    Ok(())
}

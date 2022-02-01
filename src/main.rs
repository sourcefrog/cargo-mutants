// Copyright 2021, 2022 Martin Pool

//! `cargo-mutants`: Find inadequately-tested code that can be removed without any tests failing.

mod console;
mod exit_code;
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
use crate::options::Options;
use crate::source::SourceTree;

/// Find inadequately-tested code that can be removed without any tests failing.
#[derive(FromArgs, PartialEq, Debug)]
struct Args {
    /// rust crate directory to examine.
    #[argh(option, short = 'd', default = r#"PathBuf::from(".")"#)]
    dir: PathBuf,

    /// just list possible mutants, don't run them.
    #[argh(switch)]
    list: bool,

    /// output json (only for --list).
    #[argh(switch)]
    json: bool,

    /// show the mutation diffs.
    #[argh(switch)]
    diff: bool,

    /// show cargo output for all invocations (very verbose).
    #[argh(switch)]
    all_logs: bool,

    /// cargo check generated mutants, but don't run tests.
    #[argh(switch)]
    check: bool,

    /// don't print times or tree sizes, to make output deterministic.
    #[argh(switch)]
    no_times: bool,

    /// maximum run time for all cargo commands.
    #[argh(option)]
    timeout: Option<f64>,
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
    let console = console::Console::new()
        .show_all_logs(args.all_logs)
        .show_times(!args.no_times);
    let options = Options::from(&args);
    if args.list {
        let mutations = source_tree.mutations()?;
        if args.json {
            if args.diff {
                eprintln!("--list --diff --json is not (yet) supported");
                exit(exit_code::USAGE);
            }
            serde_json::to_writer_pretty(io::BufWriter::new(io::stdout()), &mutations)?;
        } else {
            console::list_mutations(&mutations, args.diff);
        }
    } else {
        let lab_outcome = lab::test_clean_then_all_mutants(&source_tree, &options, &console)?;
        exit(lab_outcome.exit_code());
    }
    Ok(())
}

// Copyright 2021 Martin Pool

//! `cargo-mutants`: Find inadequately-tested code that can be removed without any tests failing.

mod console;
mod exit_code;
mod lab;
mod mutate;
mod outcome;
mod output;
mod source;
mod textedit;

use std::env;
use std::io;
use std::path::PathBuf;
use std::process::exit;

use anyhow::Result;
use argh::FromArgs;
#[allow(unused)]
use path_slash::PathExt;

use lab::Lab;
use source::SourceTree;

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
        let lab_outcome = Lab::new(&source_tree)?.run()?;
        exit(lab_outcome.exit_code());
    }
    Ok(())
}

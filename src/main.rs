// Copyright 2021 Martin Pool

//! `cargo-mutants`: Find inadequately-tested code that can be removed without any tests failing.

mod console;
mod lab;
mod mutate;
mod outcome;
mod output;
mod source;
mod textedit;

use std::io;
#[allow(unused)]
use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use argh::FromArgs;
#[allow(unused)]
use path_slash::PathExt;

use lab::Lab;
use source::SourceTree;

/// Find inadequately-tested code that can be removed without any tests failing.
#[derive(FromArgs, PartialEq, Debug)]
struct TopArgs {
    #[argh(subcommand)]
    command: Command,
}

// Cargo always runs external subcommands passing argv[1] as the name of the subcommand:
// <https://doc.rust-lang.org/cargo/reference/external-tools.html>
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
enum Command {
    Mutants(Mutants),
}

/// Find inadequately-tested code that can be removed without any tests failing.
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "mutants")]
struct Mutants {
    /// rust crate directory to examine.
    #[argh(option, short = 'd', default = r#"PathBuf::from(".")"#)]
    dir: PathBuf,

    /// just list possible mutants, don't run them.
    #[argh(switch)]
    list_mutants: bool,

    /// output json (only for --list-mutants).
    #[argh(switch)]
    json: bool,

    /// show the mutation diffs.
    #[argh(switch)]
    diff: bool,
}

fn main() -> Result<()> {
    let args: TopArgs = argh::from_env();
    let Command::Mutants(opts) = args.command;
    let source_tree = SourceTree::new(&opts.dir)?;
    if opts.list_mutants {
        let mutations = source_tree.mutations()?;
        if opts.json {
            // TODO: diffs in json?
            serde_json::to_writer_pretty(io::BufWriter::new(io::stdout()), &mutations)?;
        } else {
            for mutation in mutations {
                println!(
                    "{}: {}",
                    mutation.describe_location(),
                    mutation.describe_change(),
                );
                if opts.diff {
                    println!("{}", mutation.diff());
                }
            }
        }
    } else {
        Lab::new(&source_tree)?.run()?;
    }
    Ok(())
}

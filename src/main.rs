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

/// Exit codes from cargo-mutants.
///
/// These are assigned so that different cases that CI or other automation (or
/// cargo-mutants' own test suite) might want to distinguish are distinct.
///
/// These are also described in README.md.
enum ExitCode {
    #[allow(dead_code)]
    Success = 0,

    /// The wrong arguments, etc.
    /// 
    /// (1 is also the value returned by argh.)
    Usage = 1,
}

fn main() -> Result<()> {
    let args: TopArgs = argh::from_env();
    let Command::Mutants(opts) = args.command;
    let source_tree = SourceTree::new(&opts.dir)?;
    if opts.list_mutants {
        let mutations = source_tree.mutations()?;
        if opts.json {
            if opts.diff {
                eprintln!("--list-mutants --diff --json is not (yet) supported");
                exit(ExitCode::Usage as i32);
            }
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

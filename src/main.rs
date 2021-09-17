// Copyright 2021 Martin Pool

mod lab;
mod mutate;
mod source;
mod textedit;

#[allow(unused)]
use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use argh::FromArgs;
#[allow(unused)]
use path_slash::PathExt;

use lab::Lab;
use source::SourceTree;

/// Rust mutation testing.
#[derive(FromArgs, PartialEq, Debug)]
struct TopArgs {
    #[argh(subcommand)]
    command: Command,
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
enum Command {
    ListFiles(ListFiles),
    ListMutants(ListMutants),
    Test(Test),
}

/// List source files in a tree.
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "list-files")]
struct ListFiles {
    /// rust crate directory to examine.
    #[argh(option, short = 'd')]
    dir: PathBuf,
}

/// List mutation scenarios that can be generated from a file.
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "list-mutants")]
struct ListMutants {
    /// rust crate directory to examine.
    #[argh(option, short = 'd', default = r#"PathBuf::from(".")"#)]
    dir: PathBuf,
    /// show the diff between the original and mutated file.
    #[argh(switch)]
    diff: bool,
}

/// Test the tree with mutations applied.
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "test")]
struct Test {
    /// rust crate directory to examine.
    #[argh(option, short = 'd', default = r#"PathBuf::from(".")"#)]
    dir: PathBuf,
}

fn main() -> Result<()> {
    let args: TopArgs = argh::from_env();
    match args.command {
        Command::ListFiles(sub) => {
            for source_file in SourceTree::new(&sub.dir)?.source_files() {
                println!("{}", source_file.tree_relative_slashes());
            }
        }
        Command::ListMutants(sub) => {
            for source_file in SourceTree::new(&sub.dir)?.source_files() {
                for mutation in source_file.mutations()? {
                    println!(
                        "{}: {}",
                        mutation.describe_location(),
                        mutation.describe_change(),
                    );
                    if sub.diff {
                        println!("{}", mutation.diff());
                    }
                }
            }
        }
        Command::Test(sub) => {
            let source = SourceTree::new(&sub.dir)?;
            let work = Lab::new(&source)?;
            dbg!(&work);
            work.test_clean()?;
        }
    }
    Ok(())
}

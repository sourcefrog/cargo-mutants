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
use similar::TextDiff;

use lab::Lab;
use mutate::FileMutagen;
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
            for source_path in SourceTree::new(&sub.dir)?.source_files() {
                println!("{}", source_path.tree_relative_slashes());
            }
        }
        Command::ListMutants(sub) => {
            for source_path in SourceTree::new(&sub.dir)?.source_files() {
                let mutagen = FileMutagen::new(&source_path)?;
                for (i, mutation) in mutagen.discover_mutation_sites().into_iter().enumerate() {
                    println!(
                        "{:>8} {:<30} {:<16?} {}",
                        i,
                        source_path.tree_relative_slashes(),
                        mutation.op,
                        mutation.function_name()
                    );
                    if sub.diff {
                        let mutated_code = mutation.mutated_code(&mutagen);
                        let old_label = source_path.tree_relative_slashes();
                        let new_label = format!("{} {:?}", &old_label, &mutation);
                        let text_diff = TextDiff::from_lines(&mutagen.code, &mutated_code);
                        print!(
                            "{}",
                            text_diff
                                .unified_diff()
                                .context_radius(10)
                                .header(&old_label, &new_label)
                        );
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

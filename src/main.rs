// Copyright 2021 Martin Pool

mod mutate;
mod source;
mod textedit;
mod lab;

use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use argh::FromArgs;
use path_slash::PathExt;
use similar::TextDiff;

use mutate::FileMutagen;
use source::SourceTree;
use lab::Lab;

/// Rust mutation testing.
#[derive(FromArgs, PartialEq, Debug)]
struct TopArgs {
    #[argh(subcommand)]
    command: Command,
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
enum Command {
    ApplyMutation(ApplyMutation),
    ListFiles(ListFiles),
    ListMutants(ListMutants),
    Test(Test),
}

/// Print mutated source to stdout.
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "apply")]
struct ApplyMutation {
    /// rust source file to read and mutate.
    #[argh(option, short = 'f')]
    file: PathBuf,

    /// mutation index number, from list-mutants.
    #[argh(option)]
    index: usize,

    /// show the diff between the original and mutated file.
    #[argh(switch)]
    diff: bool,
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
    #[argh(option, short = 'd')]
    dir: PathBuf,
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
            for relpath in SourceTree::new(&sub.dir)?.source_files() {
                println!("{}", relpath.to_slash_lossy());
            }
        }
        Command::ListMutants(sub) => {
            for relpath in SourceTree::new(&sub.dir)?.source_files() {
                let mutagen = FileMutagen::new(sub.dir.join(&relpath))?;
                for (i, mute) in mutagen.discover_mutation_sites().into_iter().enumerate() {
                    println!(
                        "{:>8} {:<30} {:<16?} {}",
                        i,
                        relpath.to_slash_lossy(),
                        mute.op,
                        mute.function_name()
                    );
                }
            }
        }
        Command::ApplyMutation(sub) => {
            let mutagen = FileMutagen::new(sub.file)?;
            let mutation = mutagen
                .discover_mutation_sites()
                .into_iter()
                .nth(sub.index)
                .expect("index in range");
            let mutated_code = mutation.mutated_code(&mutagen);
            if sub.diff {
                let old_label = mutagen.path.to_slash_lossy();
                let new_label = format!("{} {:?}", &old_label, &mutation);
                let text_diff = TextDiff::from_lines(&mutagen.code, &mutated_code);
                print!(
                    "{}",
                    text_diff
                        .unified_diff()
                        .context_radius(10)
                        .header(&old_label, &new_label)
                );
            } else {
                std::io::stdout().write_all(mutated_code.as_bytes())?;
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

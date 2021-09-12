// Copyright 2021 Martin Pool

use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use argh::FromArgs;

mod mutate;
mod textedit;

use mutate::FileMutagen;

/// Rust mutation testing.
#[derive(FromArgs, PartialEq, Debug)]
struct TopArgs {
    #[argh(subcommand)]
    command: Command,
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
enum Command {
    ListMutants(ListMutants),
    ApplyMutation(ApplyMutation),
}

/// List mutation scenarios that can be generated from a file.
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "list-mutants")]
struct ListMutants {
    /// rust source file to examine.
    #[argh(option)]
    file: Option<PathBuf>,
}

/// Print mutated source to stdout.
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "apply")]
struct ApplyMutation {
    /// rust source file to read and mutate.
    #[argh(option)]
    file: PathBuf,

    /// mutation index number, from list-mutants.
    #[argh(option)]
    index: usize,
}

fn main() -> Result<()> {
    let args: TopArgs = argh::from_env();
    match args.command {
        Command::ListMutants(sub) => {
            let mutagen = FileMutagen::new(sub.file.expect("file must be specified"))?;
            for (i, mute) in mutagen.discover_mutation_sites().into_iter().enumerate() {
                println!("{:>8} {:<16?} {}", i, mute.op, mute.function_name());
            }
        }
        Command::ApplyMutation(sub) => {
            let mutagen = FileMutagen::new(sub.file)?;
            let mutation = mutagen
                .discover_mutation_sites()
                .into_iter()
                .nth(sub.index)
                .expect("index in range");
            std::io::stdout().write_all(mutation.mutated_code(&mutagen).as_bytes())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    // use super::*;
}

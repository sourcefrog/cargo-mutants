// Copyright 2021 Martin Pool

use std::env::args;
use std::path::PathBuf;

use anyhow::Result;

mod mutate;
mod textedit;

use mutate::FileMutagen;

fn main() -> Result<()> {
    let srcpath = PathBuf::from(&args().nth(1).expect("a Rust source file name"));
    let mutagen = FileMutagen::new(srcpath)?;
    let mutation_sites = mutagen.discover_mutation_sites();
    // eprintln!("{:#?}", mutation_sites);
    for m in &mutation_sites[..1] {
        print!("{}", m.mutated_code(&mutagen));
    }
    Ok(())
}

#[cfg(test)]
mod test {
    // use super::*;

}

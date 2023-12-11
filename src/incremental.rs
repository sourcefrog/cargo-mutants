// Copyright 2023 Paxos

//! Logic for incremantal runs
use crate::{
    mutate::{Mutant, MutantHash},
    options::Options,
    output::{PositiveOutcome, OUTDIR_NAME, POSITIVE_OUTCOMES_FILE},
};
use anyhow::{anyhow, Result};
use camino::{Utf8Path, Utf8PathBuf};
use std::{collections::HashSet, fs};

pub fn filter_by_last_positive_outcomes(
    mutants: Vec<Mutant>,
    dir: &Utf8PathBuf,
    options: &Options,
) -> (Option<Vec<PositiveOutcome>>, Vec<Mutant>) {
    let read_path: &Utf8Path = options.output_in_dir.as_ref().map_or(dir, |p| p.as_path());
    // TODO: add logging here for error cases
    let last_positive_outcomes = match read_last_positive_outcomes(read_path) {
        Ok(outcomes) => Some(outcomes),
        Err(_) => None,
    };
    // if last_positive_outcomes is none the hash set will be empty thereby allowing all mutants to be considered
    let existing_mutants: HashSet<MutantHash> = last_positive_outcomes
        .iter()
        .flatten()
        .map(|o| o.mutant_hash())
        .collect();
    let mutants = mutants
        .into_iter()
        .filter(|m| !existing_mutants.contains(&m.calculate_hash()))
        .collect();
    (last_positive_outcomes, mutants)
}

fn read_last_positive_outcomes(read_path: &Utf8Path) -> Result<Vec<PositiveOutcome>> {
    let path = read_path.join(OUTDIR_NAME).join(POSITIVE_OUTCOMES_FILE);
    fs::read_to_string(path.clone())
        .map(|contents| serde_json::from_str(&contents).map_err(|e| anyhow!("{}", e)))
        // If we can’t read the file, we assume that it doesn’t exist and we return an empty list.
        // Later, the file will get written and any error will be surfaced to the user.
        .unwrap_or(Ok(vec![]))
}

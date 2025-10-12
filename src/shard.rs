//! Sharding parameters.

use std::str::FromStr;

use anyhow::{anyhow, ensure, Context, Error};
use clap::ValueEnum;
use schemars::JsonSchema;
use serde::Deserialize;

/// Select mutants for a particular shard of the total list.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Shard {
    /// Index modulo n.
    pub k: usize,
    /// Modulus of sharding.
    pub n: usize,
}

impl FromStr for Shard {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = s.split_once('/').ok_or(anyhow!("shard must be k/n"))?;
        let k = parts.0.parse().context("shard k")?;
        let n = parts.1.parse().context("shard n")?;
        ensure!(k < n, "shard k must be less than n"); // implies n>0
        Ok(Shard { k, n })
    }
}

/// Method for sharding.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, ValueEnum, JsonSchema)]
#[serde(rename_all = "kebab-case")] // consistent with Clap default
pub enum Sharding {
    /// Run mutant `i` on shard `i % k`.
    ///
    /// This was the default up to cargo-mutants 25.3.1.
    ///
    /// This distributes mutants more evenly and will likely generate more equal completion
    /// times, but it has less locality of reference within each shard's cache and so may
    /// cause more build time.
    RoundRobin,

    /// Run consecutive ranges of mutants on each shard: the first `n/k` on shard 0, etc. (default)
    ///
    /// This makes the incremental change between each build likely to be smaller and
    /// so may reduce build time, but it may also lead to more unbalanced shards.
    #[default]
    Slice,
}

impl Sharding {
    /// Select the mutants that should be run for this shard.
    pub fn shard<M>(self, shard: Shard, mut mutants: Vec<M>) -> Vec<M> {
        match self {
            Sharding::RoundRobin => mutants
                .into_iter()
                .enumerate()
                .filter_map(|(i, m)| {
                    if i % shard.n == shard.k {
                        Some(m)
                    } else {
                        None
                    }
                })
                .collect(),
            Sharding::Slice => {
                let total = mutants.len();
                let chunk_size = total.div_ceil(shard.n);
                let start = shard.k * chunk_size;
                let end = ((shard.k + 1) * chunk_size).min(total);
                if start >= total {
                    Vec::new()
                } else {
                    mutants.drain(start..end).collect()
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shard_from_str_valid_input() {
        let shard = Shard::from_str("2/5").unwrap();
        assert_eq!(shard.k, 2);
        assert_eq!(shard.n, 5);
        assert_eq!(shard, Shard { k: 2, n: 5 });
    }

    #[test]
    fn shard_from_str_invalid_input() {
        assert_eq!(
            Shard::from_str("").unwrap_err().to_string(),
            "shard must be k/n"
        );

        assert_eq!(
            Shard::from_str("2").unwrap_err().to_string(),
            "shard must be k/n"
        );

        assert_eq!(
            Shard::from_str("2/0").unwrap_err().to_string(),
            "shard k must be less than n"
        );

        assert_eq!(
            Shard::from_str("5/2").unwrap_err().to_string(),
            "shard k must be less than n"
        );
    }

    #[test]
    fn shard_round_robin() {
        // This test works on ints instead of real mutants just for ease of testing.
        let fake_mutants: Vec<usize> = (0..10).collect();
        for (k, expect) in [
            (0, [0, 4, 8].as_slice()),
            (1, &[1, 5, 9]),
            (2, &[2, 6]),
            (3, &[3, 7]),
        ] {
            assert_eq!(
                Sharding::RoundRobin
                    .shard(Shard { k, n: 4 }, fake_mutants.clone())
                    .as_slice(),
                expect
            );
        }
    }
}

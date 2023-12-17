// Copyright 2023 Martin Pool

//! Test `--shard`

use itertools::Itertools;

use super::run;

#[test]
fn shard_divides_all_mutants() {
    // For speed, this only lists the mutants, trusting that the mutants
    // that are listed are the ones that are run.
    let common_args = ["mutants", "--list", "-d", "testdata/well_tested"];
    let full_list = String::from_utf8(
        run()
            .args(common_args)
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .unwrap()
    .lines()
    .map(ToOwned::to_owned)
    .collect_vec();

    let n_shards = 5;
    let shard_lists = (0..n_shards)
        .map(|k| {
            String::from_utf8(
                run()
                    .args(common_args)
                    .args(["--shard", &format!("{}/{}", k, n_shards)])
                    .assert()
                    .success()
                    .get_output()
                    .stdout
                    .clone(),
            )
            .unwrap()
            .lines()
            .map(ToOwned::to_owned)
            .collect_vec()
        })
        .collect_vec();

    // If you combine all the mutants selected for each shard, you get the
    // full list, with nothing lost or duplicated, disregarding order.
    //
    // If we had a bug where we shuffled before sharding, then the shards would
    // see inconsistent lists and this test would fail in at least some attempts.
    assert_eq!(
        shard_lists.iter().flatten().sorted().collect_vec(),
        full_list.iter().sorted().collect_vec()
    );

    // The shards should also be approximately the same size.
    let shard_lens = shard_lists.iter().map(|l| l.len()).collect_vec();
    assert!(shard_lens.iter().max().unwrap() - shard_lens.iter().min().unwrap() <= 1);

    // And the shards are disjoint
    for i in 0..n_shards {
        for j in 0..n_shards {
            if i != j {
                assert!(
                    shard_lists[i].iter().all(|m| !shard_lists[j].contains(m)),
                    "shard {} contains {}",
                    j,
                    shard_lists[j]
                        .iter()
                        .filter(|m| shard_lists[i].contains(m))
                        .join(", ")
                );
            }
        }
    }
}

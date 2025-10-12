// Copyright 2023-2024 Martin Pool

//! Test `--shard`

use itertools::Itertools;

mod util;
use util::{copy_of_testdata, run};

#[test]
fn shard_divides_all_mutants() {
    // For speed, this only lists the mutants, trusting that the mutants
    // that are listed are the ones that are run.
    let tmp = copy_of_testdata("well_tested");
    let common_args = ["mutants", "--list", "-d", tmp.path().to_str().unwrap()];
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
    let rr_shard_lists = (0..n_shards)
        .map(|k| {
            String::from_utf8(
                run()
                    .args(common_args)
                    .args([
                        "--shard",
                        &format!("{k}/{n_shards}"),
                        "--sharding=round-robin",
                    ])
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
        rr_shard_lists.iter().flatten().sorted().collect_vec(),
        full_list.iter().sorted().collect_vec()
    );

    // The shards should also be approximately the same size.
    let shard_lens = rr_shard_lists.iter().map(|l| l.len()).collect_vec();
    assert!(shard_lens.iter().max().unwrap() - shard_lens.iter().min().unwrap() <= 1);

    // And the shards are disjoint
    for i in 0..n_shards {
        for j in 0..n_shards {
            if i != j {
                assert!(
                    rr_shard_lists[i]
                        .iter()
                        .all(|m| !rr_shard_lists[j].contains(m)),
                    "shard {} contains {}",
                    j,
                    rr_shard_lists[j]
                        .iter()
                        .filter(|m| rr_shard_lists[j].contains(m))
                        .join(", ")
                );
            }
        }
    }

    // If you reassemble the round-robin shards in order, you get the original order back.
    //
    // To do so: cycle around the list of shards, taking one from each shard in order, until
    // we get to the end of any list.
    let mut reassembled = Vec::new();
    let mut rr_iters = rr_shard_lists
        .clone()
        .into_iter()
        .map(|l| l.into_iter())
        .collect_vec();
    let mut i = 0;
    let mut limit = 0;
    for name in rr_iters[i].by_ref() {
        reassembled.push(name);
        i = (i + 1) % n_shards;
        limit += 1;
        assert!(limit < full_list.len() * 2, "too many iterations");
    }

    // Check with slice sharding, the new default
    let slice_shard_lists = (0..n_shards)
        .map(|k| {
            String::from_utf8(
                run()
                    .args(common_args)
                    .args([&format!("--shard={k}/{n_shards}")]) //  "--sharding=slice"
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

    // These can just be concatenated
    let slice_reassembled = slice_shard_lists.into_iter().flatten().collect_vec();
    assert_eq!(slice_reassembled, full_list);
}

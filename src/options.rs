// Copyright 2021, 2022 Martin Pool

//! Global in-process options for experimenting on mutants.
//!
//! The [Options] structure is built from command-line options and then widely passed around.

use std::convert::TryFrom;
use std::time::Duration;

use anyhow::Context;
use camino::Utf8PathBuf;
use globset::{Glob, GlobSet, GlobSetBuilder};
use regex::RegexSet;
use tracing::warn;

use crate::*;

/// Options for running experiments.
#[derive(Default, Debug, Clone)]
pub struct Options {
    /// Don't run the tests, just see if each mutant builds.
    pub check_only: bool,

    /// The time limit for test tasks, if set.
    ///
    /// If this is not set by the user it's None, in which case there is no time limit
    /// on the baseline test, and then the mutated tests get a multiple of the time
    /// taken by the baseline test.
    pub test_timeout: Option<Duration>,

    pub print_caught: bool,
    pub print_unviable: bool,

    pub show_times: bool,

    /// Show logs even from mutants that were caught, or source/unmutated builds.
    pub show_all_logs: bool,

    /// Test mutants in random order.
    ///
    /// This is now the default, so that repeated partial runs are more likely to find
    /// interesting results.
    pub shuffle: bool,

    /// Additional arguments for every cargo invocation.
    pub additional_cargo_args: Vec<String>,

    /// Additional arguments to `cargo test`.
    pub additional_cargo_test_args: Vec<String>,

    /// Files to examine.
    pub examine_globset: Option<GlobSet>,

    /// Files to exclude.
    pub exclude_globset: Option<GlobSet>,

    /// Mutants to examine, as a regexp matched against the full name.
    pub examine_names: Option<RegexSet>,

    /// Mutants to skip, as a regexp matched against the full name.
    pub exclude_names: Option<RegexSet>,

    /// Create `mutants.out` within this directory (by default, the source directory).
    pub output_in_dir: Option<Utf8PathBuf>,
}

impl TryFrom<&Args> for Options {
    type Error = anyhow::Error;

    fn try_from(args: &Args) -> Result<Options> {
        if args.no_copy_target {
            warn!("--no-copy-target is deprecated and has no effect; target/ is never copied");
        }

        Ok(Options {
            additional_cargo_args: args.cargo_arg.clone(),
            additional_cargo_test_args: args.cargo_test_args.clone(),
            check_only: args.check,
            examine_names: Some(
                RegexSet::new(&args.examine_re).context("Compiling examine_re regex")?,
            ),
            examine_globset: build_glob_set(&args.file)?,
            exclude_names: Some(
                RegexSet::new(&args.exclude_re).context("Compiling exclude_re regex")?,
            ),
            exclude_globset: build_glob_set(&args.exclude)?,
            output_in_dir: args.output.clone(),
            print_caught: args.caught,
            print_unviable: args.unviable,
            shuffle: !args.no_shuffle,
            show_times: !args.no_times,
            show_all_logs: args.all_logs,
            test_timeout: args.timeout.map(Duration::from_secs_f64),
        })
    }
}

fn build_glob_set(glob_set: &Vec<String>) -> Result<Option<GlobSet>> {
    if glob_set.is_empty() {
        return Ok(None);
    }

    let mut builder = GlobSetBuilder::new();
    for glob_str in glob_set {
        if glob_str.contains('/') || glob_str.contains(std::path::MAIN_SEPARATOR) {
            builder.add(Glob::new(glob_str)?);
        } else {
            builder.add(Glob::new(&format!("**/{}", glob_str))?);
        }
    }
    Ok(Some(builder.build()?))
}

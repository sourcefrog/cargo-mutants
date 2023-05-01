// Copyright 2021-2023 Martin Pool

//! Global in-process options for experimenting on mutants.
//!
//! The [Options] structure is built from command-line options and then widely passed around.
//! Options are also merged from the [config] after reading the command line arguments.

use std::time::Duration;

use anyhow::Context;
use camino::Utf8PathBuf;
use globset::{Glob, GlobSet, GlobSetBuilder};
use regex::RegexSet;
use tracing::warn;

use crate::{config::Config, *};

/// Options for mutation testing, based on both command-line arguments and the
/// config file.
#[derive(Default, Debug, Clone)]
pub struct Options {
    /// Don't run the tests, just see if each mutant builds.
    pub check_only: bool,

    /// Don't delete scratch directories.
    pub leak_dirs: bool,

    /// The time limit for test tasks, if set.
    ///
    /// If this is not set by the user it's None, in which case there is no time limit
    /// on the baseline test, and then the mutated tests get a multiple of the time
    /// taken by the baseline test.
    pub test_timeout: Option<Duration>,

    /// The minimum test timeout, as a floor on the autoset value.
    pub minimum_test_timeout: Duration,

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

    /// Run this many `cargo build` or `cargo test` tasks in parallel.
    pub jobs: Option<usize>,

    /// Insert these values as errors from functions returning `Result`.
    pub error_values: Vec<String>,
}

impl Options {
    /// Build options by merging command-line args and config file.
    pub(crate) fn new(args: &Args, config: &Config) -> Result<Options> {
        if args.no_copy_target {
            warn!("--no-copy-target is deprecated and has no effect; target/ is never copied");
        }

        // If there's a
        let minimum_test_timeout = Duration::from_secs_f64(
            args.minimum_test_timeout
                .or(config.minimum_test_timeout)
                .unwrap_or(20f64),
        );

        Ok(Options {
            additional_cargo_args: args
                .cargo_arg
                .iter()
                .cloned()
                .chain(config.additional_cargo_args.iter().cloned())
                .collect(),
            additional_cargo_test_args: args
                .cargo_test_args
                .iter()
                .cloned()
                .chain(config.additional_cargo_test_args.iter().cloned())
                .collect(),
            check_only: args.check,
            error_values: args.error.clone(),
            examine_names: Some(
                RegexSet::new(args.examine_re.iter().chain(config.examine_re.iter()))
                    .context("Compiling examine_re regex")?,
            ),
            examine_globset: build_glob_set(args.file.iter().chain(config.examine_globs.iter()))?,
            exclude_names: Some(
                RegexSet::new(args.exclude_re.iter().chain(config.exclude_re.iter()))
                    .context("Compiling exclude_re regex")?,
            ),
            exclude_globset: build_glob_set(
                args.exclude.iter().chain(config.exclude_globs.iter()),
            )?,
            jobs: args.jobs,
            leak_dirs: args.leak_dirs,
            output_in_dir: args.output.clone(),
            print_caught: args.caught,
            print_unviable: args.unviable,
            shuffle: !args.no_shuffle,
            show_times: !args.no_times,
            show_all_logs: args.all_logs,
            test_timeout: args.timeout.map(Duration::from_secs_f64),
            minimum_test_timeout,
        })
    }
}

fn build_glob_set<S: AsRef<str>, I: IntoIterator<Item = S>>(
    glob_set: I,
) -> Result<Option<GlobSet>> {
    let mut glob_set = glob_set.into_iter().peekable();
    if glob_set.peek().is_none() {
        return Ok(None);
    }

    let mut builder = GlobSetBuilder::new();
    for glob_str in glob_set {
        let glob_str = glob_str.as_ref();
        if glob_str.contains('/') || glob_str.contains(std::path::MAIN_SEPARATOR) {
            builder.add(Glob::new(glob_str)?);
        } else {
            builder.add(Glob::new(&format!("**/{glob_str}"))?);
        }
    }
    Ok(Some(builder.build()?))
}

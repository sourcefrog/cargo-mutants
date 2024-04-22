// Copyright 2021-2024 Martin Pool

//! Global in-process options for experimenting on mutants.
//!
//! The [Options] structure is built from command-line options and then widely passed around.
//! Options are also merged from the [config] after reading the command line arguments.

use std::time::Duration;

use globset::GlobSet;
use regex::RegexSet;
use serde::Deserialize;
use strum::{Display, EnumString};
use tracing::warn;

use crate::config::Config;
use crate::glob::build_glob_set;
use crate::*;

/// Options for mutation testing, based on both command-line arguments and the
/// config file.
#[derive(Default, Debug, Clone)]
pub struct Options {
    /// Run tests in an unmutated tree?
    pub baseline: BaselineStrategy,

    /// Don't run the tests, just see if each mutant builds.
    pub check_only: bool,

    /// Don't copy files matching gitignore patterns to build directories.
    pub gitignore: bool,

    /// Don't copy at all; run tests in the source directory.
    pub in_place: bool,

    /// Don't delete scratch directories.
    pub leak_dirs: bool,

    /// The time limit for test tasks, if set.
    ///
    /// If this is not set by the user it's None, in which case there is no time limit
    /// on the baseline test, and then the mutated tests get a multiple of the time
    /// taken by the baseline test.
    pub test_timeout: Option<Duration>,

    /// The time multiplier for test tasks, if set (relative to baseline test duration).
    pub test_timeout_multiplier: Option<f64>,

    /// The time limit for build tasks, if set.
    ///
    /// If this is not set by the user it's None, in which case there is no time limit
    /// on the baseline build, and then the mutated builds get a multiple of the time
    /// taken by the baseline build.
    pub build_timeout: Option<Duration>,

    /// The time multiplier for build tasks, if set (relative to baseline build duration).
    pub build_timeout_multiplier: Option<f64>,

    /// The minimum test timeout, as a floor on the autoset value.
    pub minimum_test_timeout: Duration,

    pub print_caught: bool,
    pub print_unviable: bool,

    pub show_times: bool,

    /// Show logs even from mutants that were caught, or source/unmutated builds.
    pub show_all_logs: bool,

    /// List mutants with line and column numbers.
    pub show_line_col: bool,

    /// Test mutants in random order.
    ///
    /// This is now the default, so that repeated partial runs are more likely to find
    /// interesting results.
    pub shuffle: bool,

    /// Additional arguments for every cargo invocation.
    pub additional_cargo_args: Vec<String>,

    /// Additional arguments to `cargo test`.
    pub additional_cargo_test_args: Vec<String>,

    /// Selection of features for cargo.
    pub features: super::Features,

    /// Files to examine.
    pub examine_globset: Option<GlobSet>,

    /// Files to exclude.
    pub exclude_globset: Option<GlobSet>,

    /// Mutants to examine, as a regexp matched against the full name.
    pub examine_names: RegexSet,

    /// Mutants to skip, as a regexp matched against the full name.
    pub exclude_names: RegexSet,

    /// Create `mutants.out` within this directory (by default, the source directory).
    pub output_in_dir: Option<Utf8PathBuf>,

    /// Run this many `cargo build` or `cargo test` tasks in parallel.
    pub jobs: Option<usize>,

    /// Insert these values as errors from functions returning `Result`.
    pub error_values: Vec<String>,

    /// Show ANSI colors.
    pub colors: Colors,

    /// List mutants in json, etc.
    pub emit_json: bool,

    /// Emit diffs showing just what changed.
    pub emit_diffs: bool,

    /// The tool to use to run tests.
    pub test_tool: TestTool,
}

/// Choice of tool to use to run tests.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, EnumString, Display, Deserialize)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TestTool {
    /// Use `cargo test`, the default.
    #[default]
    Cargo,

    /// Use `cargo nextest`.
    Nextest,
}

/// Join two slices into a new vector.
fn join_slices(a: &[String], b: &[String]) -> Vec<String> {
    a.iter().chain(b).cloned().collect()
}

/// Should ANSI colors be drawn?
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Display, Deserialize, ValueEnum)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum Colors {
    #[default]
    Auto,
    Always,
    Never,
}

impl Colors {
    /// If colors were forced on or off by the user through an option or
    /// environment variable, return that value.
    ///
    /// Otherwise, return None, meaning we should decide based on the
    /// detected terminal characteristics.
    pub fn forced_value(&self) -> Option<bool> {
        // From https://bixense.com/clicolors/
        if env::var("NO_COLOR").map_or(false, |x| x != "0") {
            Some(false)
        } else if env::var("CLICOLOR_FORCE").map_or(false, |x| x != "0") {
            Some(true)
        } else {
            match self {
                Colors::Always => Some(true),
                Colors::Never => Some(false),
                Colors::Auto => None, // library should decide
            }
        }
    }

    #[mutants::skip] // depends on a real tty etc, hard to test
    pub fn active_stdout(&self) -> bool {
        self.forced_value()
            .unwrap_or_else(::console::colors_enabled)
    }
}

impl Options {
    /// Build options by merging command-line args and config file.
    pub(crate) fn new(args: &Args, config: &Config) -> Result<Options> {
        if args.no_copy_target {
            warn!("--no-copy-target is deprecated and has no effect; target/ is never copied");
        }

        let minimum_test_timeout = Duration::from_secs_f64(
            args.minimum_test_timeout
                .or(config.minimum_test_timeout)
                .unwrap_or(20f64),
        );

        let options = Options {
            additional_cargo_args: join_slices(&args.cargo_arg, &config.additional_cargo_args),
            additional_cargo_test_args: join_slices(
                &args.cargo_test_args,
                &config.additional_cargo_test_args,
            ),
            baseline: args.baseline,
            check_only: args.check,
            colors: args.colors,
            emit_json: args.json,
            emit_diffs: args.diff,
            error_values: join_slices(&args.error, &config.error_values),
            examine_names: RegexSet::new(or_slices(&args.examine_re, &config.examine_re))
                .context("Failed to compile examine_re regex")?,
            exclude_names: RegexSet::new(or_slices(&args.exclude_re, &config.exclude_re))
                .context("Failed to compile exclude_re regex")?,
            examine_globset: build_glob_set(or_slices(&args.file, &config.examine_globs))?,
            exclude_globset: build_glob_set(or_slices(&args.exclude, &config.exclude_globs))?,
            features: args.features.clone(),
            gitignore: args.gitignore,
            in_place: args.in_place,
            jobs: args.jobs,
            leak_dirs: args.leak_dirs,
            minimum_test_timeout,
            output_in_dir: args.output.clone(),
            print_caught: args.caught,
            print_unviable: args.unviable,
            shuffle: !args.no_shuffle,
            show_line_col: args.line_col,
            show_times: !args.no_times,
            show_all_logs: args.all_logs,
            test_timeout: args.timeout.map(Duration::from_secs_f64),
            test_timeout_multiplier: args.timeout_multiplier.or(config.timeout_multiplier),
            build_timeout: args.build_timeout.map(Duration::from_secs_f64),
            build_timeout_multiplier: args
                .build_timeout_multiplier
                .or(config.build_timeout_multiplier),
            test_tool: args.test_tool.or(config.test_tool).unwrap_or_default(),
        };
        options.error_values.iter().for_each(|e| {
            if e.starts_with("Err(") {
                warn!(
                    "error_value option gives the value of the error, and probably should not start with Err(: got {}",
                    e
                );
            }
        });
        Ok(options)
    }

    #[cfg(test)]
    pub fn from_args(args: &Args) -> Result<Options> {
        Options::new(args, &Config::default())
    }
}

/// If the first slices is non-empty, return that, otherwise the second.
fn or_slices<'a: 'c, 'b: 'c, 'c, T>(a: &'a [T], b: &'b [T]) -> &'c [T] {
    if a.is_empty() {
        b
    } else {
        a
    }
}

#[cfg(test)]
mod test {
    use std::io::Write;

    use indoc::indoc;
    use rusty_fork::rusty_fork_test;
    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn default_options() {
        let args = Args::parse_from(["mutants"]);
        let options = Options::new(&args, &Config::default()).unwrap();
        assert!(!options.check_only);
        assert_eq!(options.test_tool, TestTool::Cargo);
    }

    #[test]
    fn options_from_test_tool_arg() {
        let args = Args::parse_from(["mutants", "--test-tool", "nextest"]);
        let options = Options::new(&args, &Config::default()).unwrap();
        assert_eq!(options.test_tool, TestTool::Nextest);
    }

    #[test]
    fn options_from_baseline_arg() {
        let args = Args::parse_from(["mutants", "--baseline", "skip"]);
        let options = Options::new(&args, &Config::default()).unwrap();
        assert_eq!(options.baseline, BaselineStrategy::Skip);

        let args = Args::parse_from(["mutants", "--baseline", "run"]);
        let options = Options::new(&args, &Config::default()).unwrap();
        assert_eq!(options.baseline, BaselineStrategy::Run);

        let args = Args::parse_from(["mutants"]);
        let options = Options::new(&args, &Config::default()).unwrap();
        assert_eq!(options.baseline, BaselineStrategy::Run);
    }

    #[test]
    fn options_from_timeout_args() {
        let args = Args::parse_from(["mutants", "--timeout=2.0"]);
        let options = Options::new(&args, &Config::default()).unwrap();
        assert_eq!(options.test_timeout, Some(Duration::from_secs(2)));

        let args = Args::parse_from(["mutants", "--timeout-multiplier=2.5"]);
        let options = Options::new(&args, &Config::default()).unwrap();
        assert_eq!(options.test_timeout_multiplier, Some(2.5));

        let args = Args::parse_from(["mutants", "--minimum-test-timeout=60.0"]);
        let options = Options::new(&args, &Config::default()).unwrap();
        assert_eq!(options.minimum_test_timeout, Duration::from_secs(60));

        let args = Args::parse_from(["mutants", "--build-timeout=3.0"]);
        let options = Options::new(&args, &Config::default()).unwrap();
        assert_eq!(options.build_timeout, Some(Duration::from_secs(3)));

        let args = Args::parse_from(["mutants", "--build-timeout-multiplier=3.5"]);
        let options = Options::new(&args, &Config::default()).unwrap();
        assert_eq!(options.build_timeout_multiplier, Some(3.5));
    }

    #[test]
    fn cli_timeout_multiplier_overrides_config() {
        let config = indoc! { r#"
            timeout_multiplier = 1.0
            build_timeout_multiplier = 2.0
        "#};
        let mut config_file = NamedTempFile::new().unwrap();
        config_file.write_all(config.as_bytes()).unwrap();
        let args = Args::parse_from([
            "mutants",
            "--timeout-multiplier=2.0",
            "--build-timeout-multiplier=1.0",
        ]);
        let config = Config::read_file(config_file.path()).unwrap();
        let options = Options::new(&args, &config).unwrap();

        assert_eq!(options.test_timeout_multiplier, Some(2.0));
        assert_eq!(options.build_timeout_multiplier, Some(1.0));
    }

    #[test]
    fn test_tool_from_config() {
        let config = indoc! { r#"
            test_tool = "nextest"
        "#};
        let mut config_file = NamedTempFile::new().unwrap();
        config_file.write_all(config.as_bytes()).unwrap();
        let args = Args::parse_from(["mutants"]);
        let config = Config::read_file(config_file.path()).unwrap();
        let options = Options::new(&args, &config).unwrap();
        assert_eq!(options.test_tool, TestTool::Nextest);
    }

    #[test]
    fn features_arg() {
        let args = Args::try_parse_from(["mutants", "--features", "nice,shiny features"]).unwrap();
        assert_eq!(
            args.features.features.iter().as_ref(),
            ["nice,shiny features"]
        );
        assert!(!args.features.no_default_features);
        assert!(!args.features.all_features);

        let options = Options::new(&args, &Config::default()).unwrap();
        assert_eq!(
            options.features.features.iter().as_ref(),
            ["nice,shiny features"]
        );
        assert!(!options.features.no_default_features);
        assert!(!options.features.all_features);
    }

    #[test]
    fn no_default_features_arg() {
        let args = Args::try_parse_from([
            "mutants",
            "--no-default-features",
            "--features",
            "nice,shiny features",
        ])
        .unwrap();

        let options = Options::new(&args, &Config::default()).unwrap();
        assert_eq!(
            options.features.features.iter().as_ref(),
            ["nice,shiny features"]
        );
        assert!(options.features.no_default_features);
        assert!(!options.features.all_features);
    }

    #[test]
    fn all_features_arg() {
        let args = Args::try_parse_from([
            "mutants",
            "--all-features",
            "--features",
            "nice,shiny features",
        ])
        .unwrap();

        let options = Options::new(&args, &Config::default()).unwrap();
        assert_eq!(
            options.features.features.iter().as_ref(),
            ["nice,shiny features"]
        );
        assert!(!options.features.no_default_features);
        assert!(options.features.all_features);
    }

    rusty_fork_test! {
        #[test]
        fn color_control_from_cargo_env() {
            use std::env::{set_var,remove_var};

            set_var("CARGO_TERM_COLOR", "always");
            remove_var("CLICOLOR_FORCE");
            remove_var("NO_COLOR");
            let args = Args::parse_from(["mutants"]);
            let options = Options::new(&args, &Config::default()).unwrap();
            assert_eq!(options.colors.forced_value(), Some(true));

            set_var("CARGO_TERM_COLOR", "never");
            let args = Args::parse_from(["mutants"]);
            let options = Options::new(&args, &Config::default()).unwrap();
            assert_eq!(options.colors.forced_value(), Some(false));

            set_var("CARGO_TERM_COLOR", "auto");
            let args = Args::parse_from(["mutants"]);
            let options = Options::new(&args, &Config::default()).unwrap();
            assert_eq!(options.colors.forced_value(), None);

            remove_var("CARGO_TERM_COLOR");
            let args = Args::parse_from(["mutants"]);
            let options = Options::new(&args, &Config::default()).unwrap();
            assert_eq!(options.colors.forced_value(), None);
        }

        #[test]
        fn color_control_from_env() {
            use std::env::{set_var,remove_var};

            remove_var("CARGO_TERM_COLOR");
            remove_var("CLICOLOR_FORCE");
            remove_var("NO_COLOR");
            let args = Args::parse_from(["mutants"]);
            let options = Options::new(&args, &Config::default()).unwrap();
            assert_eq!(options.colors.forced_value(), None);

            remove_var("CLICOLOR_FORCE");
            set_var("NO_COLOR", "1");
            let options = Options::new(&args, &Config::default()).unwrap();
            assert_eq!(options.colors.forced_value(), Some(false));

            remove_var("NO_COLOR");
            set_var("CLICOLOR_FORCE", "1");
            let options = Options::new(&args, &Config::default()).unwrap();
            assert_eq!(options.colors.forced_value(), Some(true));

            remove_var("CLICOLOR_FORCE");
            remove_var("NO_COLOR");
            let options = Options::new(&args, &Config::default()).unwrap();
            assert_eq!(options.colors.forced_value(), None);
        }
    }
}

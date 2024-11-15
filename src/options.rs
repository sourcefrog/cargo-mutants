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
use syn::Expr;
use tracing::warn;

use crate::config::Config;
use crate::glob::build_glob_set;
use crate::*;

/// Options for mutation testing, based on both command-line arguments and the
/// config file.
#[derive(Default, Debug, Clone)]
pub struct Options<'a> {
    /// Run tests in an unmutated tree?
    pub baseline: BaselineStrategy,

    /// Turn off all lints.
    pub cap_lints: bool,

    /// Don't run the tests, just see if each mutant builds.
    pub check_only: bool,

    /// Don't copy files matching gitignore patterns to build directories.
    pub gitignore: bool,

    /// Don't copy at all; run tests in the source directory.
    pub in_place: bool,

    /// Run a jobserver to limit concurrency between child processes.
    pub jobserver: bool,

    /// Allow this many concurrent jobs, across all child processes. None means NCPU.
    pub jobserver_tasks: Option<usize>,

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

    /// Which packages to test for a given mutant.
    ///
    /// Comes from `--test-workspace` etc.
    pub test_package: TestPackages,

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

    /// Don't mutate arguments to functions or methods matching any of these name.
    ///
    /// This matches as a string against the last component of the path, so should not include
    /// `::`.
    pub skip_calls: Vec<&'a str>,

    /// Cargo profile.
    pub profile: Option<&'a str>,

    /// Additional arguments for every cargo invocation.
    pub additional_cargo_args: Vec<&'a str>,

    /// Additional arguments to `cargo test`.
    pub additional_cargo_test_args: Vec<&'a str>,

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
    pub output_in_dir: Option<&'a Utf8Path>,

    /// Run this many `cargo build` or `cargo test` tasks in parallel.
    pub jobs: Option<usize>,

    /// Insert these values as errors from functions returning `Result`.
    pub error_values: Vec<&'a str>,

    /// Show ANSI colors.
    pub colors: Colors,

    /// List mutants in json, etc.
    pub emit_json: bool,

    /// Emit diffs showing just what changed.
    pub emit_diffs: bool,

    /// The tool to use to run tests.
    pub test_tool: TestTool,
}

/// Which packages should be tested for a given mutant?
#[derive(Debug, Default, Clone, PartialEq, Eq, EnumString, Display, Deserialize)]
pub enum TestPackages {
    /// Only the package containing the mutated file.
    #[default]
    Mutated,

    /// All packages in the workspace.
    Workspace,

    /// Certain packages, specified by name.
    Named(Vec<String>),
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

impl<'a> Options<'a> {
    /// Build options by merging command-line args and config file.
    pub(crate) fn new(args: &'a Args, config: &'a Config) -> Result<Options<'a>> {
        if args.no_copy_target {
            warn!("--no-copy-target is deprecated and has no effect; target/ is never copied");
        }

        let minimum_test_timeout = Duration::from_secs_f64(
            args.minimum_test_timeout
                .or(config.minimum_test_timeout)
                .unwrap_or(20f64),
        );

        // If either command line argument is set, it overrides the config.
        let test_package = if args.test_workspace == Some(true) {
            TestPackages::Workspace
        } else if !args.test_package.is_empty() {
            TestPackages::Named(
                args.test_package
                    .iter()
                    .flat_map(|s| s.split(','))
                    .map(|s| s.to_string())
                    .collect(),
            )
        } else if args.test_workspace.is_none() && config.test_workspace == Some(true) {
            TestPackages::Workspace
        } else if !config.test_package.is_empty() {
            TestPackages::Named(config.test_package.clone())
        } else {
            TestPackages::Mutated
        };

        let options = Options {
            additional_cargo_args: args
                .cargo_arg
                .iter()
                .chain(&config.additional_cargo_args)
                .map(String::as_str)
                .collect(),
            additional_cargo_test_args: args
                .cargo_test_args
                .iter()
                .chain(&config.additional_cargo_test_args)
                .map(String::as_str)
                .collect(),
            baseline: args.baseline,
            build_timeout: args.build_timeout.map(Duration::from_secs_f64),
            build_timeout_multiplier: args
                .build_timeout_multiplier
                .or(config.build_timeout_multiplier),
            cap_lints: args.cap_lints.unwrap_or(config.cap_lints),
            check_only: args.check,
            colors: args.colors,
            emit_json: args.json,
            emit_diffs: args.diff,
            error_values: args
                .error
                .iter()
                .chain(&config.error_values)
                .map(String::as_str)
                .collect(),
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
            jobserver: args.jobserver,
            jobserver_tasks: args.jobserver_tasks,
            leak_dirs: args.leak_dirs,
            minimum_test_timeout,
            output_in_dir: args.output.as_deref(),
            print_caught: args.caught,
            print_unviable: args.unviable,
            profile: args
                .profile
                .as_ref()
                .or(config.profile.as_ref())
                .map(String::as_str),
            shuffle: !args.no_shuffle,
            show_line_col: args.line_col,
            show_times: !args.no_times,
            show_all_logs: args.all_logs,
            skip_calls: vec!["with_capacity"], // TODO: args and config
            test_package,
            test_timeout: args.timeout.map(Duration::from_secs_f64),
            test_timeout_multiplier: args.timeout_multiplier.or(config.timeout_multiplier),
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

    /// Which phases to run for each mutant.
    pub fn phases(&self) -> &[Phase] {
        if self.check_only {
            &[Phase::Check]
        } else {
            &[Phase::Build, Phase::Test]
        }
    }

    /// Return the syn ASTs for the error values, which should be inserted as return values
    /// from functions returning `Result`.
    pub(crate) fn parsed_error_exprs(&self) -> Result<Vec<Expr>> {
        self.error_values
            .iter()
            .map(|e| {
                syn::parse_str(e).with_context(|| format!("Failed to parse error value {e:?}"))
            })
            .collect()
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
    use std::str::FromStr;

    use indoc::indoc;
    use rusty_fork::rusty_fork_test;
    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn default_options() {
        let args = Args::parse_from(["mutants"]);
        let config = Config::default();
        let options = Options::new(&args, &config).unwrap();
        assert!(!options.check_only);
        assert_eq!(options.test_tool, TestTool::Cargo);
        assert!(!options.cap_lints);
    }

    #[test]
    fn options_from_test_tool_arg() {
        let args = Args::parse_from(["mutants", "--test-tool", "nextest"]);
        let config = Config::default();
        let options = Options::new(&args, &config).unwrap();
        assert_eq!(options.test_tool, TestTool::Nextest);
    }

    #[test]
    fn options_from_baseline_arg() {
        let args = Args::parse_from(["mutants", "--baseline", "skip"]);
        let config = Config::default();
        let options = Options::new(&args, &config).unwrap();
        assert_eq!(options.baseline, BaselineStrategy::Skip);

        let args = Args::parse_from(["mutants", "--baseline", "run"]);
        let config = Config::default();
        let options = Options::new(&args, &config).unwrap();
        assert_eq!(options.baseline, BaselineStrategy::Run);

        let args = Args::parse_from(["mutants"]);
        let config = Config::default();
        let options = Options::new(&args, &config).unwrap();
        assert_eq!(options.baseline, BaselineStrategy::Run);
    }

    #[test]
    fn options_from_timeout_args() {
        let args = Args::parse_from(["mutants", "--timeout=2.0"]);
        let config = Config::default();
        let options = Options::new(&args, &config).unwrap();
        assert_eq!(options.test_timeout, Some(Duration::from_secs(2)));

        let args = Args::parse_from(["mutants", "--timeout-multiplier=2.5"]);
        let options = Options::new(&args, &config).unwrap();
        assert_eq!(options.test_timeout_multiplier, Some(2.5));

        let args = Args::parse_from(["mutants", "--minimum-test-timeout=60.0"]);
        let options = Options::new(&args, &config).unwrap();
        assert_eq!(options.minimum_test_timeout, Duration::from_secs(60));

        let args = Args::parse_from(["mutants", "--build-timeout=3.0"]);
        let options = Options::new(&args, &config).unwrap();
        assert_eq!(options.build_timeout, Some(Duration::from_secs(3)));

        let args = Args::parse_from(["mutants", "--build-timeout-multiplier=3.5"]);
        let options = Options::new(&args, &config).unwrap();
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
    fn conflicting_timeout_options() {
        let args = Args::try_parse_from(["mutants", "--timeout=1", "--timeout-multiplier=1"])
            .expect_err("--timeout and --timeout-multiplier should conflict");
        let rendered = format!("{}", args.render());
        assert!(rendered.contains("error: the argument '--timeout <TIMEOUT>' cannot be used with '--timeout-multiplier <TIMEOUT_MULTIPLIER>'"));
    }

    #[test]
    fn conflicting_build_timeout_options() {
        let args = Args::try_parse_from([
            "mutants",
            "--build-timeout=1",
            "--build-timeout-multiplier=1",
        ])
        .expect_err("--build-timeout and --build-timeout-multiplier should conflict");
        let rendered = format!("{}", args.render());
        assert!(rendered.contains("error: the argument '--build-timeout <BUILD_TIMEOUT>' cannot be used with '--build-timeout-multiplier <BUILD_TIMEOUT_MULTIPLIER>'"));
    }

    #[test]
    fn from_config() {
        let config = indoc! { r#"
            test_tool = "nextest"
            cap_lints = true
        "#};
        let mut config_file = NamedTempFile::new().unwrap();
        config_file.write_all(config.as_bytes()).unwrap();
        let args = Args::parse_from(["mutants"]);
        let config = Config::read_file(config_file.path()).unwrap();
        let options = Options::new(&args, &config).unwrap();
        assert_eq!(options.test_tool, TestTool::Nextest);
        assert!(options.cap_lints);
    }

    #[test]
    fn features_arg() {
        let args = Args::try_parse_from(["mutants", "--features", "nice,shiny features"]).unwrap();
        let config = Config::default();
        assert_eq!(
            args.features.features.iter().as_ref(),
            ["nice,shiny features"]
        );
        assert!(!args.features.no_default_features);
        assert!(!args.features.all_features);

        let options = Options::new(&args, &config).unwrap();
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
        let config = Config::default();

        let options = Options::new(&args, &config).unwrap();
        assert_eq!(
            options.features.features.iter().as_ref(),
            ["nice,shiny features"]
        );
        assert!(options.features.no_default_features);
        assert!(!options.features.all_features);
    }

    #[test]
    fn default_jobserver_settings() {
        let args = Args::parse_from(["mutants"]);
        let config = Config::default();
        let options = Options::new(&args, &config).unwrap();
        assert!(options.jobserver);
        assert_eq!(options.jobserver_tasks, None);
    }

    #[test]
    fn disable_jobserver() {
        let args = Args::parse_from(["mutants", "--jobserver=false"]);
        let config = Config::default();
        let options = Options::new(&args, &config).unwrap();
        assert!(!options.jobserver);
        assert_eq!(options.jobserver_tasks, None);
    }

    #[test]
    fn jobserver_tasks() {
        let args = Args::parse_from(["mutants", "--jobserver-tasks=13"]);
        let config = Config::default();
        let options = Options::new(&args, &config).unwrap();
        assert!(options.jobserver);
        assert_eq!(options.jobserver_tasks, Some(13));
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
        let config = Config::default();

        let options = Options::new(&args, &config).unwrap();
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
        let config = Config::default();
            let options = Options::new(&args, &config).unwrap();
            assert_eq!(options.colors.forced_value(), Some(true));

            set_var("CARGO_TERM_COLOR", "never");
            let args = Args::parse_from(["mutants"]);
            let options = Options::new(&args, &config).unwrap();
            assert_eq!(options.colors.forced_value(), Some(false));

            set_var("CARGO_TERM_COLOR", "auto");
            let args = Args::parse_from(["mutants"]);
            let options = Options::new(&args, &config).unwrap();
            assert_eq!(options.colors.forced_value(), None);

            remove_var("CARGO_TERM_COLOR");
            let args = Args::parse_from(["mutants"]);
            let options = Options::new(&args, &config).unwrap();
            assert_eq!(options.colors.forced_value(), None);
        }

        #[test]
        fn color_control_from_env() {
            use std::env::{set_var,remove_var};

            remove_var("CARGO_TERM_COLOR");
            remove_var("CLICOLOR_FORCE");
            remove_var("NO_COLOR");
            let args = Args::parse_from(["mutants"]);
        let config = Config::default();
            let options = Options::new(&args, &config).unwrap();
            assert_eq!(options.colors.forced_value(), None);

            remove_var("CLICOLOR_FORCE");
            set_var("NO_COLOR", "1");
            let options = Options::new(&args, &config).unwrap();
            assert_eq!(options.colors.forced_value(), Some(false));

            remove_var("NO_COLOR");
            set_var("CLICOLOR_FORCE", "1");
            let options = Options::new(&args, &config).unwrap();
            assert_eq!(options.colors.forced_value(), Some(true));

            remove_var("CLICOLOR_FORCE");
            remove_var("NO_COLOR");
            let options = Options::new(&args, &config).unwrap();
            assert_eq!(options.colors.forced_value(), None);
        }
    }

    #[test]
    fn profile_option_from_args() {
        let args = Args::parse_from(["mutants", "--profile=mutants"]);
        let config = Config::default();
        let options = Options::new(&args, &config).unwrap();
        assert_eq!(options.profile.unwrap(), "mutants");
    }

    #[test]
    fn profile_from_config() {
        let args = Args::parse_from(["mutants", "-j3"]);
        let config = indoc! { r#"
                profile = "mutants"
                timeout_multiplier = 1.0
                build_timeout_multiplier = 2.0
            "#};
        let mut config_file = NamedTempFile::new().unwrap();
        config_file.write_all(config.as_bytes()).unwrap();
        let config = Config::read_file(config_file.path()).unwrap();
        let options = Options::new(&args, &config).unwrap();
        assert_eq!(options.profile.unwrap(), "mutants");
    }

    #[test]
    fn test_workspace_arg_true() {
        let args = Args::parse_from(["mutants", "--test-workspace=true"]);
        let config = Config::default();
        let options = Options::new(&args, &config).unwrap();
        assert_eq!(options.test_package, TestPackages::Workspace);
    }

    #[test]
    fn test_workspace_arg_false() {
        let args = Args::parse_from(["mutants", "--test-workspace=false"]);
        let config = Config::default();
        let options = Options::new(&args, &config).unwrap();
        assert_eq!(options.test_package, TestPackages::Mutated);
    }

    #[test]
    fn test_workspace_config_true() {
        let args = Args::parse_from(["mutants"]);
        let config = indoc! { r#"
                test_workspace = true
            "#};
        let config = Config::from_str(config).unwrap();
        let options = Options::new(&args, &config).unwrap();
        assert_eq!(options.test_package, TestPackages::Workspace);
    }

    #[test]
    fn test_workspace_config_false() {
        let args = Args::parse_from(["mutants"]);
        let config = indoc! { r#"
                test_workspace = false
            "#};
        let config = Config::from_str(config).unwrap();
        let options = Options::new(&args, &config).unwrap();
        assert_eq!(options.test_package, TestPackages::Mutated);
    }

    #[test]
    fn test_workspace_args_override_config_true() {
        let args = Args::parse_from(["mutants", "--test-workspace=true"]);
        let config = indoc! { r#"
                test_workspace = false
            "#};
        let config = Config::from_str(config).unwrap();
        let options = Options::new(&args, &config).unwrap();
        assert_eq!(options.test_package, TestPackages::Workspace);
    }

    #[test]
    fn test_workspace_args_override_config_false() {
        let args = Args::parse_from(["mutants", "--test-workspace=false"]);
        let config = indoc! { r#"
                test_workspace = true
            "#};
        let config = Config::from_str(config).unwrap();
        let options = Options::new(&args, &config).unwrap();
        assert_eq!(options.test_package, TestPackages::Mutated);
    }

    #[test]
    fn test_workspace_arg_false_allows_packages_from_config() {
        let args = Args::parse_from(["mutants", "--test-workspace=false"]);
        let config = indoc! { r#"
                # Normally the packages would be ignored, but --test-workspace=false.
                test_workspace = true
                test_package = ["foo", "bar"]
            "#};
        let config = Config::from_str(config).unwrap();
        let options = Options::new(&args, &config).unwrap();
        assert_eq!(
            options.test_package,
            TestPackages::Named(vec!["foo".to_string(), "bar".to_string()])
        );
    }

    #[test]
    fn test_package_arg_with_commas() {
        let args = Args::parse_from(["mutants", "--test-package=foo,bar"]);
        let config = Config::default();
        let options = Options::new(&args, &config).unwrap();
        assert_eq!(
            options.test_package,
            TestPackages::Named(vec!["foo".to_string(), "bar".to_string()])
        );
    }
}

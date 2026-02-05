// Copyright 2021-2026 Martin Pool

//! `cargo-mutants`: Find test gaps by inserting bugs.
//!
//! See <https://mutants.rs> for the manual and more information.

#![warn(clippy::pedantic)]
#![allow(
    clippy::module_name_repetitions,
    clippy::needless_raw_string_hashes,
    clippy::too_many_lines
)]

mod annotation;
mod build_dir;
mod cargo;
mod config;
mod console;
mod copy_tree;
mod exit_code;
mod fnvalue;
mod glob;
mod in_diff;
mod interrupt;
mod lab;
mod list;
mod manifest;
mod mutant;
mod options;
mod outcome;
mod output;
mod package;
mod path;
mod pretty;
mod process;
mod scenario;
mod shard;
mod source;
mod span;
mod tail_file;
#[cfg(test)]
#[path = "../tests/util/mod.rs"]
mod test_util;
mod timeouts;
mod visit;
mod workspace;

use std::env;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result, anyhow, bail};
use camino::{Utf8Path, Utf8PathBuf};
use clap::{
    ArgAction, CommandFactory, Parser, ValueEnum,
    builder::{Styles, styling},
};
use clap_complete::{Shell, generate};
use color_print::cstr;
use console::enable_console_colors;
use tracing::{debug, error, info};

use crate::{
    build_dir::BuildDir,
    console::Console,
    in_diff::diff_filter_file,
    interrupt::check_interrupted,
    lab::test_mutants,
    list::{list_files, list_mutants},
    mutant::{Genre, Mutant},
    options::{Colors, Common, Options},
    outcome::{Phase, ScenarioOutcome},
    output::{OutputDir, load_previously_caught},
    package::Package,
    scenario::Scenario,
    shard::Shard,
    source::SourceFile,
    visit::walk_file,
    workspace::{PackageFilter, Workspace},
};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const NAME: &str = env!("CARGO_PKG_NAME");

/// A comment marker inserted next to changes, so they can be easily found.
static MUTATION_MARKER_COMMENT: &str = "/* ~ changed by cargo-mutants ~ */";

static SPONSOR_MESSAGE: &str = cstr!(
    "<magenta><bold>Support and accelerate cargo-mutants at <<https://github.com/sponsors/sourcefrog>></></>"
);

#[mutants::skip] // only visual effects, not worth testing
fn clap_styles() -> Styles {
    styling::Styles::styled()
        .header(styling::AnsiColor::Green.on_default() | styling::Effects::BOLD)
        .usage(styling::AnsiColor::Green.on_default() | styling::Effects::BOLD)
        .literal(styling::AnsiColor::Blue.on_default() | styling::Effects::BOLD)
        .placeholder(styling::AnsiColor::Cyan.on_default())
}

#[derive(Parser)]
#[command(name = "cargo", bin_name = "cargo", styles(clap_styles()))]
enum Cargo {
    #[command(name = "mutants", styles(clap_styles()))]
    Mutants(Args),
}

#[derive(Debug, Default, ValueEnum, Clone, Copy, Eq, PartialEq)]
pub enum BaselineStrategy {
    /// Run tests in an unmutated tree before testing mutants.
    #[default]
    Run,

    /// Don't run tests in an unmutated tree: assume that they pass.
    Skip,
}

#[derive(Debug, ValueEnum, Clone, Copy, Eq, PartialEq)]
pub enum SchemaType {
    /// Emit JSON schema for the configuration file format.
    Config,
}

/// Find inadequately-tested code that can be removed without any tests failing.
///
/// See <https://mutants.rs/> for more information.
#[allow(clippy::struct_excessive_bools, clippy::struct_field_names)]
#[derive(Parser, PartialEq, Debug)]
#[command(
    author,
    about,
    after_help = SPONSOR_MESSAGE,
    styles(clap_styles())
)]
pub struct Args {
    // Note: Please keep args grouped within the source by their "help_heading", with the headings
    // in alphabetical order, and args in order within their heading, so they can be easily
    // navigated and so that related args occur near each other in the source.

    // Build ==========
    /// Turn off all rustc lints, so that denied warnings won't make mutants unviable.
    #[arg(long, action = ArgAction::Set, help_heading = "Build")]
    cap_lints: Option<bool>,

    /// Build with this cargo profile.
    #[arg(long, help_heading = "Build")]
    profile: Option<String>,

    // Config ============================================================
    /// Read configuration from this file instead of .cargo/mutants.toml.
    #[arg(
        long,
        help_heading = "Config",
        value_name = "FILE",
        conflicts_with = "no_config"
    )]
    config: Option<Utf8PathBuf>,

    /// Don't read .cargo/mutants.toml.
    #[arg(long, help_heading = "Config", conflicts_with = "config")]
    no_config: bool,

    // Copying ==========
    /// Copy the /target directory to build directories.
    #[arg(long, help_heading = "Copying", group = "copy_opts")]
    copy_target: Option<bool>,

    /// Copy `.git` and other VCS directories to the build directory.
    ///
    /// This is useful if you have tests that depend on the presence of these directories.
    ///
    /// Known VCS directories are
    /// `.git`, `.hg`, `.bzr`, `.svn`, `_darcs`, `.pijul`.
    #[arg(long, help_heading = "Copying", visible_alias = "copy_git")]
    copy_vcs: Option<bool>,

    /// Don't copy files matching gitignore patterns.
    #[arg(long, help_heading = "Copying", group = "copy_opts")]
    gitignore: Option<bool>,

    /// Test mutations in the source tree, rather than in a copy.
    #[arg(
        long,
        help_heading = "Copying",
        conflicts_with = "jobs",
        conflicts_with = "copy_opts"
    )]
    in_place: bool,

    /// Don't copy the /target directory, and don't build the source tree first.
    #[arg(long, help_heading = "Copying", group = "copy_opts", hide = true)]
    no_copy_target: bool,

    // Debug ==========
    /// Don't delete the scratch directories, for debugging.
    #[arg(long, help_heading = "Debug")]
    leak_dirs: bool,

    /// Log level for stdout (trace, debug, info, warn, error).
    #[arg(
        long,
        short = 'L',
        default_value = "info",
        env = "CARGO_MUTANTS_TRACE_LEVEL",
        help_heading = "Debug"
    )]
    level: tracing::Level,

    /// Mutate one Rust source file and list the mutants generated.
    ///
    /// The file need not be in a workspace, and the workspace manifest (if any) and configuration are ignored.
    ///
    /// A configuration file can be specified with the `--config` option.
    ///
    /// This is intended for debugging and testing mutant generation.
    #[arg(
        help_heading = "Debug",
        value_name = "FILE",
        long = "Zmutate-file",
        conflicts_with = "in_diff",
        conflicts_with = "package"
    )]
    mutate_file: Option<PathBuf>,

    // Execution ==========
    /// Baseline strategy: check that tests pass in an unmutated tree before testing mutants.
    #[arg(long, value_enum, default_value_t = BaselineStrategy::Run, help_heading = "Execution")]
    baseline: BaselineStrategy,

    /// Build timeout multiplier (relative to base build time).
    #[arg(long, help_heading = "Execution", conflicts_with = "build_timeout")]
    build_timeout_multiplier: Option<f64>,

    /// Maximum run time for cargo build command, in seconds.
    #[arg(long, help_heading = "Execution")]
    build_timeout: Option<f64>,

    /// Additional args for all cargo invocations.
    #[arg(
        long,
        short = 'C',
        allow_hyphen_values = true,
        help_heading = "Execution"
    )]
    cargo_arg: Vec<String>,

    /// Additional args for cargo test.
    #[arg(long, allow_hyphen_values = true, help_heading = "Execution")]
    cargo_test_arg: Vec<String>,

    /// Pass remaining arguments to cargo test after all options and after `--`.
    #[arg(last = true, help_heading = "Execution")]
    cargo_test_args: Vec<String>,

    /// Cargo check generated mutants, but don't run tests.
    #[arg(long, help_heading = "Execution")]
    check: bool,

    /// Run this many cargo build/test jobs in parallel.
    #[arg(
        long,
        short = 'j',
        env = "CARGO_MUTANTS_JOBS",
        help_heading = "Execution"
    )]
    jobs: Option<usize>,

    /// Use a GNU Jobserver to cap concurrency between child processes.
    #[arg(long, action = ArgAction::Set, help_heading = "Execution", default_value_t = true)]
    jobserver: bool,

    /// Allow this many jobserver tasks in parallel, across all child processes.
    ///
    /// By default, NCPUS.
    #[arg(long, help_heading = "Execution")]
    jobserver_tasks: Option<usize>,

    /// Just list possible mutants, don't run them.
    #[arg(long, help_heading = "Execution")]
    list: bool,

    /// List source files, don't run anything.
    #[arg(long, help_heading = "Execution")]
    list_files: bool,

    /// Minimum timeout for tests, in seconds, as a lower bound on the auto-set time.
    #[arg(
        long,
        env = "CARGO_MUTANTS_MINIMUM_TEST_TIMEOUT",
        help_heading = "Execution"
    )]
    minimum_test_timeout: Option<f64>,

    /// Run mutants in the fixed order they occur in the source tree.
    ///
    /// This is now the default behavior.
    #[arg(long, help_heading = "Execution")]
    no_shuffle: bool,

    /// Run only one shard of all generated mutants: specify as e.g. 1/4.
    #[arg(long, help_heading = "Execution")]
    shard: Option<Shard>,

    /// Run mutants in random order.
    ///
    /// Randomization occurs after sharding: each shard will run its assigned mutants
    /// in random order.
    #[arg(long, help_heading = "Execution", conflicts_with = "no_shuffle")]
    shuffle: bool,

    /// Maximum run time for all cargo commands, in seconds.
    #[arg(long, short = 't', help_heading = "Execution")]
    timeout: Option<f64>,

    /// Test timeout multiplier (relative to base test time).
    #[arg(long, help_heading = "Execution", conflicts_with = "timeout")]
    timeout_multiplier: Option<f64>,

    // Features ============================================================
    /// Space or comma separated list of features to activate.
    // (The features are not split or parsed, just passed through to Cargo.)
    #[arg(long, help_heading = "Features")]
    pub features: Vec<String>,

    /// Do not activate the `default` feature.
    #[arg(long, help_heading = "Features")]
    pub no_default_features: bool,

    /// Activate all features.
    // (This does not conflict because this only turns on features in the top level package,
    // and you might use --features to turn on features in dependencies.)
    #[arg(long, help_heading = "Features")]
    pub all_features: bool,

    // Filters ============================================================
    /// Regex for mutations to examine, matched against the names shown by `--list`.
    #[arg(
        long = "re",
        short = 'F',
        alias = "regex",
        alias = "examine-regex",
        alias = "examine-re",
        help_heading = "Filters"
    )]
    examine_re: Vec<String>,

    /// Glob for files to exclude; with no glob, all files are included; globs containing
    /// slash match the entire path. If used together with `--file` argument, then the files to be examined are matched before the files to be excluded.
    #[arg(long, short = 'e', help_heading = "Filters")]
    exclude: Vec<String>,

    /// Regex for mutations to exclude, matched against the names shown by `--list`.
    #[arg(long, short = 'E', alias = "exclude-regex", help_heading = "Filters")]
    exclude_re: Vec<String>,

    /// Glob for files to examine; with no glob, all files are examined; globs containing
    /// slash match the entire path. If used together with `--exclude` argument, then the files to be examined are matched before the files to be excluded.
    #[arg(long, short = 'f', help_heading = "Filters")]
    file: Vec<String>,

    /// Include only mutants in code touched by this diff.
    #[arg(long, short = 'D', help_heading = "Filters")]
    in_diff: Option<Utf8PathBuf>,

    /// Skip mutants that were caught in previous runs.
    #[arg(long, help_heading = "Filters")]
    iterate: bool,

    /// Only test mutants from these packages.
    #[arg(id = "package", long, short = 'p', help_heading = "Filters")]
    mutate_packages: Vec<String>,

    /// Skip calls to functions and methods named in this list.
    ///
    /// The list may contain comma-separated names and may be repeated.
    ///
    /// If a qualified path is given in the source then this matches only the final component,
    /// and it ignores type parameters.
    ///
    /// This value is combined with the names from the config `skip_calls` key.
    #[arg(long, help_heading = "Filters")]
    skip_calls: Vec<String>,

    /// Use built-in defaults for `skip_calls`, in addition to any explicit values.
    ///
    /// The default is `with_capacity`.
    #[arg(long, help_heading = "Filters")]
    skip_calls_defaults: Option<bool>,

    /// Generate mutations in every package in the workspace.
    #[arg(long, help_heading = "Filters")]
    workspace: bool,

    // Generate ============================================================
    /// Return this error values from functions returning Result: for example, `::anyhow::anyhow!("mutated")`.
    #[arg(long, help_heading = "Generate")]
    error: Vec<String>,

    // Input ============================================================
    /// Rust crate directory to examine.
    #[arg(
        long,
        short = 'd',
        conflicts_with = "manifest_path",
        help_heading = "Input"
    )]
    dir: Option<Utf8PathBuf>,

    /// Path to Cargo.toml for the package to mutate.
    #[arg(long, help_heading = "Input")]
    manifest_path: Option<Utf8PathBuf>,

    // Meta ============================================================
    /// Generate autocompletions for the given shell.
    #[arg(long, help_heading = "Meta")]
    completions: Option<Shell>,

    /// Show version and quit.
    #[arg(long, action = clap::ArgAction::SetTrue, help_heading = "Meta")]
    version: bool,

    // Output ============================================================
    /// Show cargo output for all invocations (very verbose).
    #[arg(long, help_heading = "Output")]
    all_logs: bool,

    /// Emit annotations for code review tools.
    #[arg(long, help_heading = "Output", default_value = "auto")]
    annotations: annotation::AutoAnnotation,

    /// Print mutants that were caught by tests.
    #[arg(long, short = 'v', help_heading = "Output")]
    caught: bool,

    /// Draw colors in output.
    #[arg(
        long,
        value_enum,
        help_heading = "Output",
        default_value_t,
        env = "CARGO_TERM_COLOR"
    )]
    colors: Colors,

    /// Output json (only for --list).
    #[arg(long, help_heading = "Output")]
    json: bool,

    /// Include line & column numbers in the mutation list.
    #[arg(long, action = ArgAction::Set, default_value = "true", help_heading = "Output")]
    line_col: bool,

    /// Don't print times or tree sizes, to make output deterministic.
    #[arg(long, help_heading = "Output")]
    no_times: bool,

    /// Create mutants.out within this directory.
    #[arg(
        long,
        short = 'o',
        env = "CARGO_MUTANTS_OUTPUT",
        help_heading = "Output"
    )]
    output: Option<Utf8PathBuf>,

    /// Print mutations that failed to check or build.
    #[arg(long, short = 'V', help_heading = "Output")]
    unviable: bool,

    // Tests ============================================================
    /// Run tests from these packages for all mutants.
    #[arg(long, help_heading = "Tests")]
    test_package: Vec<String>,

    /// Run all tests in the workspace.
    ///
    /// If false, only the tests in the mutated package are run.
    ///
    /// Overrides `--test_package`.
    #[arg(long, help_heading = "Tests")]
    test_workspace: Option<bool>,

    // Misc ============================================================
    /// Emit a JSON schema for the specified format and exit.
    #[arg(long, value_enum, help_heading = "Misc")]
    emit_schema: Option<SchemaType>,

    // Common definitions between config file and command line.
    #[clap(flatten)]
    common: Common,
}

fn main() -> Result<ExitCode> {
    let args = match Cargo::try_parse() {
        Ok(Cargo::Mutants(args)) => args,
        Err(e) => {
            e.print().expect("Failed to show clap error message");
            // Clap by default exits with code 2.
            let code = match e.exit_code() {
                2 => exit_code::USAGE,
                0 => 0,
                _ => exit_code::SOFTWARE,
            };
            return Ok(exit_code::code_to_exit_code(code));
        }
    };

    if args.version {
        println!("{NAME} {VERSION}");
        return Ok(ExitCode::SUCCESS);
    } else if let Some(shell) = args.completions {
        generate(shell, &mut Cargo::command(), "cargo", &mut io::stdout());
        return Ok(ExitCode::SUCCESS);
    } else if let Some(schema_type) = args.emit_schema {
        emit_schema(schema_type)?;
        return Ok(ExitCode::SUCCESS);
    }

    let console = Console::new();
    console.setup_global_trace(args.level, args.colors); // We don't have Options yet.
    enable_console_colors(args.colors);
    interrupt::install_handler();

    if let Some(path) = &args.mutate_file {
        // Don't use tree config here, I think?
        let config = if let Some(config_path) = &args.config {
            config::Config::read_file(config_path.as_ref())?
        } else {
            config::Config::default()
        };
        let options = Options::new(&args, &config)?;
        mutate_file(path, &options)?;
        return Ok(ExitCode::SUCCESS);
    }

    let start_dir: &Utf8Path = if let Some(manifest_path) = &args.manifest_path {
        if !manifest_path.is_file() {
            bail!("Manifest path is not a file");
        }
        manifest_path
            .parent()
            .context("Manifest path has no parent")?
    } else if let Some(dir) = &args.dir {
        dir
    } else {
        Utf8Path::new(".")
    };
    let workspace = Workspace::open(start_dir)?;
    let config = if args.no_config {
        config::Config::default()
    } else if let Some(config_path) = &args.config {
        config::Config::read_file(config_path.as_ref())?
    } else {
        config::Config::read_tree_config(workspace.root())?
    };
    debug!(?config);
    debug!(?args.features);
    let options = Options::new(&args, &config)?;
    debug!(?options);
    let package_filter = if !args.mutate_packages.is_empty() {
        PackageFilter::explicit(&args.mutate_packages)
    } else if args.workspace {
        PackageFilter::All
    } else {
        PackageFilter::Auto(start_dir.to_owned())
    };

    let output_parent_dir = options
        .output_in_dir
        .clone()
        .unwrap_or_else(|| workspace.root().to_owned());

    let mut discovered = workspace.discover(&package_filter, &options, &console)?;

    let previously_caught = if args.iterate {
        let previously_caught = load_previously_caught(&output_parent_dir)?;
        info!(
            "Iteration excludes {} previously caught or unviable mutants",
            previously_caught.len()
        );
        discovered.remove_previously_caught(&previously_caught);
        Some(previously_caught)
    } else {
        None
    };

    console.clear();
    if args.list_files {
        print!("{}", list_files(&discovered.files, &options));
        return Ok(ExitCode::SUCCESS);
    }
    let mut mutants = discovered.mutants;
    if let Some(diff_path) = &args.in_diff {
        mutants = match diff_filter_file(mutants, diff_path) {
            Ok(mutants) => mutants,
            Err(err) => {
                if err.exit_code() == 0 {
                    info!("{err}");
                } else {
                    error!("{err}");
                }
                return Ok(exit_code::code_to_exit_code(err.exit_code()));
            }
        };
    }
    if let Some(shard) = &args.shard {
        mutants = options.sharding().shard(*shard, mutants);
    }
    if args.list {
        print!("{}", list_mutants(&mutants, &options));
        Ok(ExitCode::SUCCESS)
    } else {
        let output_dir = OutputDir::new(&output_parent_dir)?;
        if let Some(previously_caught) = previously_caught {
            output_dir.write_previously_caught(&previously_caught)?;
        }
        console.set_debug_log(output_dir.open_debug_log()?);
        let lab_outcome = test_mutants(mutants, &workspace, output_dir, &options, &console)?;
        Ok(exit_code::code_to_exit_code(lab_outcome.exit_code()))
    }
}

fn emit_schema(schema_type: SchemaType) -> Result<()> {
    match schema_type {
        SchemaType::Config => {
            let schema = schemars::schema_for!(config::Config);
            println!("{}", serde_json::to_string_pretty(&schema)?);
            Ok(())
        }
    }
}

/// Mutate one file, that does not need to be in a Cargo workspace, and list the mutants generated,
/// as either text or JSON.
fn mutate_file(path: &Path, options: &Options) -> Result<()> {
    let fake_package = Package {
        name: "single_file".to_string(),
        version: "0.0.0".to_string(),
        relative_dir: Utf8PathBuf::new(),
        top_sources: Vec::new(),
    };
    let path = Utf8PathBuf::from_path_buf(path.to_owned())
        .map_err(|_| anyhow!("mutate_file path is not UTF-8"))?;
    let source_file = SourceFile::load(
        path.parent().context("get parent of mutate_file")?,
        path.file_name()
            .context("get file name of mutate_file")?
            .into(),
        &fake_package,
        true,
    )
    .context("load source file")?
    .context("single source file is outside of tree??")?;
    let error_exprs = options.parsed_error_exprs()?;
    let (mutants, _mod_refs) = walk_file(&source_file, &error_exprs, options)?;
    print!("{}", list_mutants(&mutants, options));
    Ok(())
}

#[cfg(test)]
mod test {
    use clap::{CommandFactory, Parser};

    #[test]
    fn config_option_conflicts_with_no_config() {
        let args = super::Args::try_parse_from(["mutants", "--config=foo.toml", "--no-config"]);
        assert!(args.is_err(), "Expected error due to conflicting options");
        println!("Error message: {}", args.unwrap_err());
    }

    #[test]
    fn option_help_sentence_case_without_period() {
        let args = super::Args::command();
        let mut problems = Vec::new();
        for arg in args.get_arguments() {
            if let Some(help) = arg.get_help().map(ToString::to_string) {
                if !help.starts_with(char::is_uppercase) {
                    problems.push(format!(
                        "Help for {:?} does not start with a capital letter: {:?}",
                        arg.get_id(),
                        help
                    ));
                }
                // Clap seems to automatically strip periods from the end of help text in docstrings,
                // but let's leave this here just in case.
                if help.ends_with('.') {
                    problems.push(format!(
                        "Help for {:?} ends with a period: {:?}",
                        arg.get_id(),
                        help
                    ));
                }
                if help.is_empty() {
                    problems.push(format!("Help for {:?} is empty", arg.get_id()));
                }
            } else {
                problems.push(format!("No help for {:?}", arg.get_id()));
            }
        }
        for problem in &problems {
            eprintln!("{problem}");
        }
        assert!(problems.is_empty(), "Problems with help text");
    }
}

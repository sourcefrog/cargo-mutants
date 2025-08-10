// Copyright 2022-2025 Martin Pool.

//! `.cargo/mutants.toml` configuration file.
//!
//! The config file is read after parsing command line arguments,
//! and after finding the source tree, because these together
//! determine its location.
//!
//! The config file is then merged in to the [`Options`].

use std::default::Default;
use std::fs::read_to_string;
use std::path::Path;
use std::str::FromStr;

use anyhow::Context;
use camino::Utf8Path;
use schemars::JsonSchema;
use serde::Deserialize;
use tracing::debug;

use crate::options::TestTool;
use crate::Result;

// NOTE: Docstrings on this struct and its members turn into descriptions in the JSON schema,
// so keep them focused on the externally-visible behavior.

#[derive(Debug, Default, Clone, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(extend("$id" = "https://json.schemastore.org/cargo-mutants-config.json"))]
#[schemars(title = "cargo-mutants configuration")]
#[schemars(
    description = "cargo-mutants configuration, read by default from `.cargo/mutants.toml`. See <https://mutants.rs/>."
)]
pub struct Config {
    /// Pass extra args to every cargo invocation.
    pub additional_cargo_args: Vec<String>,
    /// Pass extra args to cargo test.
    pub additional_cargo_test_args: Vec<String>,
    /// Activate all features.
    pub all_features: Option<bool>,
    /// Build timeout multiplier, relative to the baseline 'cargo build'.
    pub build_timeout_multiplier: Option<f64>,

    /// Pass `--cap-lints` to rustc.
    pub cap_lints: bool,
    /// Copy `.git` and other VCS directories to the build directory.
    pub copy_vcs: Option<bool>,
    /// Copy the /target directory to build directories.
    pub copy_target: Option<bool>,
    /// Generate these error values from functions returning Result.
    pub error_values: Vec<String>,
    /// Generate mutants from source files matching these globs.
    pub examine_globs: Vec<String>,
    /// Examine only mutants matching these regexps.
    pub examine_re: Vec<String>,
    /// Exclude mutants from source files matching these globs.
    pub exclude_globs: Vec<String>,
    /// Exclude mutants from source files matches these regexps.
    pub exclude_re: Vec<String>,

    /// When copying the tree, exclude patterns in `.gitignore`.
    pub gitignore: Option<bool>,

    /// Space or comma separated list of features to activate.
    pub features: Vec<String>,
    /// Minimum test timeout, in seconds, as a floor on the autoset value.
    pub minimum_test_timeout: Option<f64>,
    /// Do not activate the `default` feature.
    pub no_default_features: Option<bool>,
    /// Output directory.
    pub output: Option<String>,
    /// Cargo profile.
    pub profile: Option<String>,
    /// Skip calls to functions or methods with these names.
    ///
    /// This is combined with values from the --skip-calls argument.
    pub skip_calls: Vec<String>,
    /// Use built-in defaults for `skip_calls` in addition to any explicit values.
    pub skip_calls_defaults: Option<bool>,
    /// Run tests from these packages for all mutants.
    pub test_package: Vec<String>,
    /// Choice of test tool: cargo or nextest.
    pub test_tool: Option<TestTool>,
    /// Run tests from all packages in the workspace, not just the mutated package.
    ///
    /// Overrides `test_package`.
    pub test_workspace: Option<bool>,
    /// Timeout multiplier, relative to the baseline 'cargo test'.
    pub timeout_multiplier: Option<f64>,
}

impl Config {
    pub fn read_file(path: &Path) -> Result<Config> {
        debug!(?path, "Read config");
        let toml =
            read_to_string(path).with_context(|| format!("read config {}", path.display()))?;
        Config::from_str(&toml).with_context(|| format!("parse toml from {}", path.display()))
    }

    /// Read the config from a tree's `.cargo/mutants.toml`, and return a default (empty)
    /// Config is the file does not exist.
    pub fn read_tree_config(workspace_dir: &Utf8Path) -> Result<Config> {
        let path = workspace_dir.join(".cargo").join("mutants.toml");
        if path.exists() {
            debug!(?path, "Found config in source tree");
            Config::read_file(path.as_ref())
        } else {
            debug!("No config found in workspace");
            Ok(Config::default())
        }
    }
}

impl FromStr for Config {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        toml::de::from_str(s).with_context(|| "parse toml")
    }
}

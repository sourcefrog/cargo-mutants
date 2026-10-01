// Copyright 2022-2026 Martin Pool.

//! `mutants.toml` configuration file.
//!
//! The config is read after parsing command line arguments,
//! and after finding the source tree, because these together
//! determine its location.
//!
//! Within the tree, the config is read from the first of
//! [`CONFIG_FILE_NAMES`] that exists, or otherwise from a
//! `[workspace.metadata.mutants]` or `[package.metadata.mutants]`
//! table in the root `Cargo.toml`.
//!
//! The config is then merged in to the [`Options`].

use std::default::Default;
use std::fs::read_to_string;
use std::path::Path;
use std::str::FromStr;

use anyhow::Context;
use camino::Utf8Path;
use schemars::JsonSchema;
use serde::Deserialize;
use tracing::debug;

use crate::Result;
use crate::options::Common;

// NOTE: Docstrings on this struct and its members turn into descriptions in the JSON schema,
// so keep them focused on the externally-visible behavior.

#[derive(Debug, Default, Clone, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(extend("$id" = "https://json.schemastore.org/cargo-mutants-config.json"))]
#[schemars(title = "cargo-mutants configuration")]
#[schemars(
    description = "cargo-mutants configuration, read by default from `.cargo/mutants.toml` or another standard location in the source tree. See <https://mutants.rs/>."
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
    /// Examine only mutants whose name matches one of these regexps.
    pub examine_re: Vec<String>,
    /// Exclude mutants from source files matching these globs.
    pub exclude_globs: Vec<String>,
    /// Exclude mutants whose name matches these regexps.
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

    /// Run tests from all packages in the workspace, not just the mutated package.
    ///
    /// Overrides `test_package`.
    pub test_workspace: Option<bool>,
    /// Timeout multiplier, relative to the baseline 'cargo test'.
    pub timeout_multiplier: Option<f64>,

    // Common definition of options that can be set from both the command line and the config file.
    #[serde(flatten)]
    pub common: Common,
}

/// Paths, relative to the root of the source tree, from which the config is read,
/// in order of precedence: the first one that exists is used.
const CONFIG_FILE_NAMES: &[&str] = &[
    ".cargo/mutants.toml",
    "mutants.toml",
    ".mutants.toml",
    ".config/mutants.toml",
];

/// Tables in the root `Cargo.toml` from which the config is read if none of
/// [`CONFIG_FILE_NAMES`] exists, in order of precedence.
const MANIFEST_METADATA_TABLES: &[&[&str]] = &[
    &["workspace", "metadata", "mutants"],
    &["package", "metadata", "mutants"],
];

impl Config {
    pub fn read_file(path: &Path) -> Result<Config> {
        debug!(?path, "Read config");
        let toml =
            read_to_string(path).with_context(|| format!("read config {}", path.display()))?;
        Config::from_str(&toml).with_context(|| format!("parse toml from {}", path.display()))
    }

    /// Read the config from the first of the standard locations in a tree that
    /// exists, and return a default (empty) Config if there is none.
    pub fn read_tree_config(workspace_dir: &Utf8Path) -> Result<Config> {
        for name in CONFIG_FILE_NAMES {
            let path = workspace_dir.join(name);
            if path.is_file() {
                debug!(?path, "Found config in source tree");
                return Config::read_file(path.as_ref());
            }
        }
        if let Some(config) = Config::read_manifest_metadata(workspace_dir)? {
            return Ok(config);
        }
        debug!("No config found in workspace");
        Ok(Config::default())
    }

    /// Read the config from a metadata table in the tree's root `Cargo.toml`,
    /// if there is one.
    fn read_manifest_metadata(workspace_dir: &Utf8Path) -> Result<Option<Config>> {
        let path = workspace_dir.join("Cargo.toml");
        if !path.is_file() {
            return Ok(None);
        }
        let toml = read_to_string(&path).with_context(|| format!("read manifest {path}"))?;
        let manifest: toml::Table =
            toml::de::from_str(&toml).with_context(|| format!("parse toml from {path}"))?;
        for keys in MANIFEST_METADATA_TABLES {
            let mut value = manifest.get(keys[0]);
            for key in &keys[1..] {
                value = value.and_then(|value| value.get(key));
            }
            if let Some(value) = value {
                let table = keys.join(".");
                debug!(?path, table, "Found config in manifest");
                return Config::deserialize(value.clone())
                    .with_context(|| format!("parse `[{table}]` from {path}"))
                    .map(Some);
            }
        }
        Ok(None)
    }
}

impl FromStr for Config {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        toml::de::from_str(s).with_context(|| "parse toml")
    }
}

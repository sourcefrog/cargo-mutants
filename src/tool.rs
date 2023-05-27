// Copyright 2023 Martin Pool.

//! A generic interface for running a tool such as Cargo that can determine the tree
//! shape, build it, and run tests.
//!
//! At present only Cargo is supported, but this interface aims to leave a place to
//! support for example Bazel in future.

use std::fmt::Debug;
use std::marker::{Send, Sync};
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
#[allow(unused_imports)]
use tracing::{debug, debug_span, trace};

use crate::options::Options;
use crate::outcome::Phase;
use crate::scenario::Scenario;
use crate::SourceFile;
use crate::{build_dir, Result};

pub trait Tool: Debug + Send + Sync {
    fn name(&self) -> &str;

    /// Find the root of the package enclosing a given path.
    ///
    /// The root is the enclosing directory that needs to be copied to make a self-contained
    /// scratch directory, and from where source discovery begins.
    fn find_root(&self, path: &Utf8Path) -> Result<Utf8PathBuf>;

    /// Find all the root files from whence source discovery should begin.
    ///
    /// For Cargo, this is files like `src/bin/*.rs`, `src/lib.rs` identified by targets
    /// in the manifest.
    fn root_files(&self, path: &Utf8Path) -> Result<Vec<Arc<SourceFile>>>;

    /// Compose argv to run one phase in this tool.
    fn compose_argv(
        &self,
        build_dir: &build_dir::BuildDir,
        scenario: &Scenario,
        phase: Phase,
        options: &Options,
    ) -> Result<Vec<String>>;

    fn compose_env(
        &self,
        scenario: &Scenario,
        phase: Phase,
        options: &Options,
    ) -> Result<Vec<(String, String)>>;
}

// Copyright 2023 Martin Pool.

//! A generic interface for running a tool such as Cargo that can determine the tree
//! shape, build it, and run tests.
//!
//! At present only Cargo is supported, but this interface aims to leave a place to
//! support for example Bazel in future.

use std::fmt::Debug;
use std::marker::{Send, Sync};
use std::sync::Arc;

use camino::Utf8Path;
#[allow(unused_imports)]
use tracing::{debug, debug_span, trace};

use crate::options::Options;
use crate::outcome::Phase;
use crate::source::Package;
use crate::SourceFile;
use crate::{build_dir, Result};

pub trait Tool: Debug + Send + Sync {
    /// A short name for this tool, like "cargo".
    fn name(&self) -> &str;

    /// Find the top-level files for each package within a tree.
    ///
    /// The path is the root returned by [find_root].
    ///
    /// For Cargo, this is files like `src/bin/*.rs`, `src/lib.rs` identified by targets
    /// in the manifest for each package.
    ///
    /// From each of these top files, we can discover more source by following `mod`
    /// statements.
    ///
    /// If `packages` is non-empty, include only packages whose name is in this list.
    fn top_source_files(
        &self,
        path: &Utf8Path,
        packages: &[String],
    ) -> Result<Vec<Arc<SourceFile>>>;

    /// Compose argv to run one phase in this tool.
    fn compose_argv(
        &self,
        build_dir: &build_dir::BuildDir,
        packages: Option<&[&Package]>,
        phase: Phase,
        options: &Options,
    ) -> Result<Vec<String>>;

    fn compose_env(&self) -> Result<Vec<(String, String)>>;
}

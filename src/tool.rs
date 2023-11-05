// Copyright 2023 Martin Pool.

//! A generic interface for running a tool such as Cargo that can determine the tree
//! shape, build it, and run tests.
//!
//! At present only Cargo is supported, but this interface aims to leave a place to
//! support for example Bazel in future.

use std::fmt::Debug;
use std::marker::{Send, Sync};

#[allow(unused_imports)]
use tracing::{debug, debug_span, trace};

use crate::options::Options;
use crate::outcome::Phase;
use crate::source::Package;
use crate::{build_dir, Result};

pub trait Tool: Debug + Send + Sync {
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

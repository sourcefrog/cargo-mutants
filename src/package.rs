// Copyright 2023 Martin Pool

//! Discover and represent cargo packages within a workspace.

use camino::Utf8PathBuf;

/// A package built and tested as a unit.
#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub struct Package {
    /// The short name of the package, like "mutants".
    pub name: String,

    /// For Cargo, the path of the `Cargo.toml` manifest file, relative to the top of the tree.
    pub relative_manifest_path: Utf8PathBuf,
}

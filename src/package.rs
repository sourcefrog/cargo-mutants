// Copyright 2023-2024 Martin Pool

//! Discover and represent cargo packages within a workspace.

use camino::Utf8PathBuf;

/// A package built and tested as a unit.
#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub struct Package {
    /// The short name of the package, like "mutants".
    pub name: String,

    /// For Cargo, the path of the `Cargo.toml` manifest file, relative to the top of the tree.
    pub relative_manifest_path: Utf8PathBuf,

    /// The top source files for this package, relative to the workspace root,
    /// like `["src/lib.rs"]`.
    pub top_sources: Vec<Utf8PathBuf>,
}

/// A more specific view of which packages to mutate, after resolving `PackageFilter::Auto`.
#[derive(Debug, Clone)]
#[allow(clippy::module_name_repetitions)]
pub enum PackageSelection {
    All,
    Explicit(Vec<String>),
}

impl PackageSelection {
    /// Helper constructor for `PackageSelection::Explicit`.
    pub fn explicit<I: IntoIterator<Item = S>, S: ToString>(names: I) -> Self {
        Self::Explicit(names.into_iter().map(|s| s.to_string()).collect())
    }
}

// Copyright 2023-2025 Martin Pool

//! Discover and represent cargo packages within a workspace.

use camino::Utf8PathBuf;

/// A package built and tested as a unit.
#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub struct Package {
    /// The short name of the package, like "mutants".
    pub name: String,

    /// The directory for this package relative to the workspace.
    ///
    /// For a package in the root, this is `""`.
    pub relative_dir: Utf8PathBuf,

    /// The top source files for this package, relative to the workspace root,
    /// like `["src/lib.rs"]`.
    pub top_sources: Vec<Utf8PathBuf>,
}

/// A more specific view of which packages to mutate, after resolving `PackageFilter::Auto`.
#[derive(Debug, Clone)]
#[allow(clippy::module_name_repetitions)]
pub enum PackageSelection {
    All,
    /// Explicitly selected packages, by qualname.
    Explicit(Vec<String>),
}

impl PackageSelection {
    /// Helper constructor for `PackageSelection::Explicit`.
    pub fn explicit<I: IntoIterator<Item = S>, S: ToString>(names: I) -> Self {
        Self::Explicit(names.into_iter().map(|s| s.to_string()).collect())
    }
}

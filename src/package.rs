// Copyright 2023-2025 Martin Pool

//! Discover and represent cargo packages within a workspace.

use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::TargetKind;
use itertools::Itertools;
use serde::Serialize;
use tracing::{debug, debug_span, warn};

/// A package built and tested as a unit.
#[derive(Debug, Eq, PartialEq, Hash, Clone, Serialize)]
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

/// Read `cargo-metadata` parsed output, and produce our package representation.
pub fn packages_from_metadata(metadata: &cargo_metadata::Metadata) -> Vec<Package> {
    metadata
        .workspace_packages()
        .into_iter()
        .sorted_by_key(|p| &p.name)
        .filter_map(|p| Package::from_cargo_metadata(p, &metadata.workspace_root))
        .collect()
}

impl Package {
    pub fn from_cargo_metadata(
        package_metadata: &cargo_metadata::Package,
        workspace_root: &Utf8Path,
    ) -> Option<Self> {
        let name = package_metadata.name.clone();
        let _span = debug_span!("package", %name).entered();
        let manifest_path = &package_metadata.manifest_path;
        debug!(%manifest_path, "walk package");
        let Some(relative_dir) = manifest_path
            .strip_prefix(workspace_root)
            .ok()
            .and_then(|p| p.parent())
            .map(ToOwned::to_owned)
        else {
            warn!(
                "manifest path {manifest_path:?} for package {name:?} is not within \
                the detected source root path {workspace_root:?} or has no parent"
            );
            return None;
        };
        Some(Package {
            name,
            top_sources: package_top_sources(workspace_root, package_metadata),
            relative_dir,
        })
    }
}

/// Find all the files that are named in the `path` of targets in a
/// Cargo manifest, if the kind of the target is one that we should mutate.
///
/// These are the starting points for discovering source files.
fn package_top_sources(
    workspace_root: &Utf8Path,
    package_metadata: &cargo_metadata::Package,
) -> Vec<Utf8PathBuf> {
    let mut found = Vec::new();
    let pkg_dir = package_metadata.manifest_path.parent().unwrap();
    for target in &package_metadata.targets {
        if should_mutate_target(target) {
            if let Ok(relpath) = target
                .src_path
                .strip_prefix(workspace_root)
                .map(ToOwned::to_owned)
            {
                debug!(
                    "found mutation target {} of kind {:?}",
                    relpath, target.kind
                );
                found.push(relpath);
            } else {
                warn!("{:?} is not in {:?}", target.src_path, pkg_dir);
            }
        } else {
            debug!(
                "skipping target {:?} of kinds {:?}",
                target.name, target.kind
            );
        }
    }
    found.sort();
    found.dedup();
    found
}

fn should_mutate_target(target: &cargo_metadata::Target) -> bool {
    target.kind.iter().any(|kind| {
        matches!(
            kind,
            TargetKind::Bin
                | TargetKind::ProcMacro
                | TargetKind::CDyLib
                | TargetKind::DyLib
                | TargetKind::Lib
                | TargetKind::RLib
                | TargetKind::StaticLib
        )
    })
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

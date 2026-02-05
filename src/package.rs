// Copyright 2023-2025 Martin Pool

//! Discover and represent cargo packages within a workspace.

use std::sync::Arc;

use std::path::{Path, PathBuf};
use cargo_metadata::TargetKind;
use itertools::Itertools;
use serde::Serialize;
use tracing::{debug, debug_span, warn};

/// A package built and tested as a unit.
///
/// This is an internal representation derived from and similar to a `cargo_metadata::Package`,
/// in a more digested form and as an extension point for later supporting tools other than Cargo.
#[derive(Debug, Eq, PartialEq, Hash, Clone, Serialize)]
pub struct Package {
    /// The short name of the package, like "mutants".
    pub name: String,

    /// The version of the package, like `"0.1.0"`.
    pub version: String,

    /// The directory for this package relative to the workspace.
    ///
    /// For a package in the root, this is `""`.
    pub relative_dir: PathBuf,

    /// The top source files for this package, relative to the workspace root,
    /// like `["src/lib.rs"]`.
    pub top_sources: Vec<PathBuf>,
}

/// Read `cargo-metadata` parsed output, and produce our package representation.
pub fn packages_from_metadata(metadata: &cargo_metadata::Metadata) -> Vec<Arc<Package>> {
    let workspace_root = PathBuf::from(metadata.workspace_root.as_str());
    metadata
        .workspace_packages()
        .into_iter()
        .sorted_by_key(|p| &p.name)
        .filter_map(|p| Package::from_cargo_metadata(p, &workspace_root))
        .map(Arc::new)
        .collect()
}

impl Package {
    pub fn from_cargo_metadata(
        package_metadata: &cargo_metadata::Package,
        workspace_root: &Path,
    ) -> Option<Self> {
        let name = package_metadata.name.clone();
        let _span = debug_span!("package", %name).entered();
        let manifest_path = &package_metadata.manifest_path;
        debug!(%manifest_path, "walk package");
        let Some(relative_dir) = manifest_path
            .strip_prefix(workspace_root)
            .ok()
            .and_then(|p| p.parent())
            .map(|p| PathBuf::from(p.as_str()))
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
            version: package_metadata.version.to_string(),
            relative_dir,
        })
    }

    pub fn version_qualified_name(&self) -> String {
        format!("{}@{}", self.name, self.version)
    }
}

/// Find all the files that are named in the `path` of targets in a
/// Cargo manifest, if the kind of the target is one that we should mutate.
///
/// These are the starting points for discovering source files.
fn package_top_sources(
    workspace_root: &Path,
    package_metadata: &cargo_metadata::Package,
) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let pkg_dir = package_metadata.manifest_path.parent().unwrap();
    for target in &package_metadata.targets {
        if should_mutate_target(target) {
            if let Ok(relpath) = target
                .src_path
                .strip_prefix(workspace_root)
                .map(|p| PathBuf::from(p.as_str()))
            {
                debug!(
                    "found mutation target {relpath:?} of kind {kind:?}",
                    kind = target.kind
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

/// Selection of which specific packages to mutate or test.
#[derive(Debug, Clone)]
#[allow(clippy::module_name_repetitions)]
pub enum PackageSelection {
    /// All packages in the workspace.
    All,
    /// Explicitly selected packages.
    Explicit(Vec<Arc<Package>>),
}

impl PackageSelection {
    #[cfg(test)]
    pub fn one<P: Into<PathBuf>>(
        name: &str,
        version: &str,
        relative_dir: P,
        top_source: &str,
    ) -> Self {
        Self::Explicit(vec![Arc::new(Package {
            name: name.to_string(),
            version: version.to_string(),
            relative_dir: relative_dir.into(),
            top_sources: vec![top_source.into()],
        })])
    }
}

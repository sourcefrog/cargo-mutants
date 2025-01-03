// Copyright 2023-2024 Martin Pool

//! Understand cargo workspaces, which can contain multiple packages.
//!
//! In cargo-mutants there are a few important connections to workspaces:
//!
//! 1. We copy the whole workspace to scratch directories, so need to find the root.
//!
//! 2. We can select to mutate, or run tests from, all packages in the workspace,
//!    or just some, so we need to find the packages. Also, mutants are marked with the
//!    package they come from.
//!
//! 3. In particular when selecting packages, we attempt to match cargo's own heuristics
//!    when invoked inside a workspace.

#![warn(clippy::pedantic)]

use std::fmt;
use std::panic::catch_unwind;
use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, bail, ensure, Context};
use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::{Metadata, TargetKind};
use itertools::Itertools;
use serde_json::Value;
use tracing::{debug, debug_span, error, warn};

use crate::cargo::cargo_bin;
use crate::console::Console;
use crate::interrupt::check_interrupted;
use crate::options::{Options, TestPackages};
use crate::package::{Package, PackageSelection};
use crate::visit::{walk_tree, Discovered};
use crate::Result;

/// Which packages to mutate in a workspace?
#[derive(Debug, Clone)]
pub enum PackageFilter {
    /// Include every package in the workspace.
    All,
    /// Packages with given names, from `--package`.
    Explicit(Vec<String>),
    /// Automatic behavior when invoked from a subdirectory.
    ///
    /// This tries to match
    /// <https://doc.rust-lang.org/cargo/reference/workspaces.html#package-selection>.
    ///
    /// If the directory is within a package directory, select that package.
    ///
    /// Otherwise, this is a "virtual workspace" directory, containing members but no
    /// primary package. In this case, if there is a `default-members` field in the workspace,
    /// use that list. Otherwise, apply to all members of the workspace.
    Auto(Utf8PathBuf),
}

impl PackageFilter {
    /// Convenience constructor for `PackageFilter::Explicit`.
    pub fn explicit<S: ToString, I: IntoIterator<Item = S>>(names: I) -> PackageFilter {
        PackageFilter::Explicit(names.into_iter().map(|s| s.to_string()).collect_vec())
    }

    /// Translate an auto package filter to either All or Explicit.
    fn resolve_auto(&self, metadata: &cargo_metadata::Metadata) -> Result<PackageSelection> {
        match &self {
            PackageFilter::Auto(dir) => {
                // Find the closest package directory (with a cargo manifest) to the current directory.
                let package_dir = locate_project(dir, false)?;
                assert!(package_dir.is_absolute());
                // It's not required that the members be inside the workspace directory: see
                // <https://doc.rust-lang.org/cargo/reference/workspaces.html>
                for package in metadata.workspace_packages() {
                    // If this package is one of the workspace members, then select this package.
                    if package.manifest_path.parent().expect("remove Cargo.toml") == package_dir {
                        debug!("resolved auto package filter to {:?}", package.name);
                        return Ok(PackageSelection::Explicit(vec![package.name.clone()]));
                    }
                }
                // Otherwise, we're in a virtual workspace directory, and not inside any package.
                // Use configured defaults if there are any, otherwise test all packages.
                let workspace_dir = &metadata.workspace_root;
                ensure!(
                    &package_dir == workspace_dir,
                    "package {package_dir:?} doesn't match any child and doesn't match the workspace root {workspace_dir:?}?",
                );
                Ok(workspace_default_packages(metadata))
            }
            PackageFilter::All => Ok(PackageSelection::All),
            PackageFilter::Explicit(names) => Ok(PackageSelection::Explicit(names.clone())),
        }
    }
}

/// Return the default workspace packages.
///
/// Default packages can be specified in the workspace's `Cargo.toml` file;
/// if not, all packages are included.
fn workspace_default_packages(metadata: &Metadata) -> PackageSelection {
    // `cargo_metadata::workspace_default_packages` will panic when calling Cargo older than 1.71;
    // in that case we'll just fall back to everything, for lack of a better option.
    match catch_unwind(|| metadata.workspace_default_packages()) {
        Ok(default_packages) => {
            if default_packages.is_empty() {
                PackageSelection::All
            } else {
                PackageSelection::Explicit(
                    default_packages
                        .into_iter()
                        .map(|pmeta| pmeta.name.clone())
                        .collect(),
                )
            }
        }
        Err(err) => {
            warn!(
                cargo_metadata_error = err.downcast::<String>().unwrap_or_default(),
                "workspace_default_packages is not supported; testing all packages",
            );
            PackageSelection::All
        }
    }
}

/// A cargo workspace.
pub struct Workspace {
    metadata: cargo_metadata::Metadata,
    packages: Vec<Package>,
}

impl fmt::Debug for Workspace {
    #[mutants::skip]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The `metadata` value is very large so is omitted here;
        // just the root is enough.
        f.debug_struct("Workspace")
            .field("root", &self.root().to_string())
            .finish_non_exhaustive()
    }
}

impl Workspace {
    /// The root directory of the workspace.
    pub fn root(&self) -> &Utf8Path {
        &self.metadata.workspace_root
    }

    /// Open the workspace containing a given directory.
    pub fn open<P: AsRef<Path>>(start_dir: P) -> Result<Self> {
        let start_dir = start_dir.as_ref();
        let dir = locate_project(start_dir.try_into().expect("start_dir is UTF-8"), true)?;
        assert!(
            dir.is_absolute(),
            "project location {dir:?} is not absolute"
        );
        let manifest_path = dir.join("Cargo.toml");
        debug!(?manifest_path, "Find root files");
        check_interrupted()?;
        let metadata = cargo_metadata::MetadataCommand::new()
            .no_deps()
            .manifest_path(&manifest_path)
            .verbose(false)
            .exec()
            .with_context(|| format!("Failed to run cargo metadata on {manifest_path}"))?;
        debug!(workspace_root = ?metadata.workspace_root, "Found workspace root");
        let packages = packages_from_metadata(&metadata)?;
        Ok(Workspace { metadata, packages })
    }

    pub fn has_package(&self, name: &str) -> bool {
        self.packages.iter().any(|p| p.name == name)
    }

    pub fn check_test_packages_are_present(&self, test_package: &TestPackages) -> Result<()> {
        if let TestPackages::Named(test_package) = test_package {
            let missing = test_package
                .iter()
                .filter(|&name| !self.has_package(name))
                .collect_vec();
            if !missing.is_empty() {
                bail!(
                    "Some package names in --test-package are not present in the workspace: {}",
                    missing.into_iter().join(", ")
                );
            }
        }
        Ok(())
    }
    /// Find packages matching some filter.
    fn packages(&self, package_filter: &PackageFilter) -> Result<Vec<Package>> {
        match package_filter.resolve_auto(&self.metadata)? {
            PackageSelection::Explicit(wanted) => {
                let packages = self
                    .packages
                    .iter()
                    .filter(|p| wanted.contains(&p.name))
                    .cloned()
                    .collect_vec();
                for wanted in wanted {
                    if !packages.iter().any(|package| package.name == *wanted) {
                        warn!("package {wanted:?} not found in source tree");
                    }
                }
                Ok(packages)
            }
            PackageSelection::All => Ok(self.packages.iter().cloned().collect_vec()),
        }
    }

    /// Make all the mutants from the filtered packages in this workspace.
    pub fn discover(
        &self,
        package_filter: &PackageFilter,
        options: &Options,
        console: &Console,
    ) -> Result<Discovered> {
        walk_tree(
            self.root(),
            &self.packages(package_filter)?,
            options,
            console,
        )
    }
}

/// Read `cargo-metadata` parsed output, and produce our package representation.
fn packages_from_metadata(metadata: &Metadata) -> Result<Vec<Package>> {
    let mut packages = Vec::new();
    let root = &metadata.workspace_root;
    for package_metadata in metadata
        .workspace_packages()
        .into_iter()
        .sorted_by_key(|p| &p.name)
    {
        check_interrupted()?;
        let name = &package_metadata.name;
        let _span = debug_span!("package", %name).entered();
        let manifest_path = &package_metadata.manifest_path;
        debug!(%manifest_path, "walk package");
        let relative_dir = manifest_path
                .strip_prefix(root)
                .map_err(|_| {
                    // TODO: Maybe just warn and skip?
                    anyhow!(
                        "manifest path {manifest_path:?} for package {name:?} is not within the detected source root path {root:?}",
                    )
                })?
                .parent()
                .expect("remove Cargo.toml")
                .to_owned();
        packages.push(Package {
            name: package_metadata.name.clone(),
            top_sources: package_top_sources(root, package_metadata),
            relative_dir,
        });
    }
    Ok(packages)
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
    for kind in &target.kind {
        // bin / proc-macro / *lib
        if matches!(
            kind,
            TargetKind::Bin
                | TargetKind::ProcMacro
                | TargetKind::CDyLib
                | TargetKind::DyLib
                | TargetKind::Lib
                | TargetKind::RLib
                | TargetKind::StaticLib
        ) {
            return true;
        }
    }
    false
}

/// Return the path of the workspace or package directory enclosing a given directory.
fn locate_project(path: &Utf8Path, workspace: bool) -> Result<Utf8PathBuf> {
    ensure!(path.is_dir(), "{path:?} is not a directory");
    let mut args: Vec<&str> = vec!["locate-project"];
    if workspace {
        args.push("--workspace");
    }
    let output = Command::new(cargo_bin())
        .args(&args)
        .current_dir(path)
        .output()
        .with_context(|| format!("failed to spawn {args:?}"))?;
    let exit = output.status;
    if !exit.success() {
        error!(
            ?exit,
            "cargo locate-project failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        bail!("cargo locate-project failed");
    }
    let stdout =
        String::from_utf8(output.stdout).context("cargo locate-project output is not UTF-8")?;
    debug!("output: {}", stdout.trim());
    let val: Value = serde_json::from_str(&stdout).context("parse cargo locate-project output")?;
    let cargo_toml_path: Utf8PathBuf = val["root"]
        .as_str()
        .with_context(|| format!("cargo locate-project output has no root: {stdout:?}"))?
        .to_owned()
        .into();
    debug!(?cargo_toml_path, "Found workspace root manifest");
    ensure!(
        cargo_toml_path.is_file(),
        "cargo locate-project root {cargo_toml_path:?} is not a file"
    );
    let root = cargo_toml_path
        .parent()
        .ok_or_else(|| anyhow!("cargo locate-project root {cargo_toml_path:?} has no parent"))?
        .to_owned();
    ensure!(
        root.is_dir(),
        "apparent project root directory {root:?} is not a directory"
    );
    Ok(root)
}

#[cfg(test)]
mod test {
    use camino::Utf8PathBuf;
    use itertools::Itertools;

    use crate::console::Console;
    use crate::options::Options;
    use crate::test_util::copy_of_testdata;
    use crate::workspace::PackageFilter;

    use super::Workspace;

    #[test]
    fn error_opening_outside_of_crate() {
        Workspace::open("/").unwrap_err();
    }

    #[test]
    fn open_subdirectory_of_crate_opens_the_crate() {
        let tmp = copy_of_testdata("factorial");
        let workspace = Workspace::open(&tmp).expect("open source tree from subdirectory");
        let root = workspace.root();
        assert!(root.is_dir());
        assert!(root.join("Cargo.toml").is_file());
        assert!(root.join("src/bin/factorial.rs").is_file());
    }

    #[test]
    fn find_root_from_subdirectory_of_workspace_finds_the_workspace_root() {
        let tmp = copy_of_testdata("workspace");
        let workspace = Workspace::open(tmp.path()).expect("Find root from within workspace/main");
        let root = workspace.root();
        assert_eq!(
            root.canonicalize().unwrap(),
            tmp.path().canonicalize().unwrap()
        );
    }

    #[test]
    fn find_top_source_files_from_subdirectory_of_workspace() {
        let tmp = copy_of_testdata("workspace");
        let workspace = Workspace::open(tmp.path()).expect("Find workspace root");
        let packages = workspace.packages(&PackageFilter::All).unwrap();
        assert_eq!(packages[0].name, "cargo_mutants_testdata_workspace_utils");
        assert_eq!(packages[0].top_sources, ["utils/src/lib.rs"]);
        assert_eq!(packages[1].name, "main");
        assert_eq!(packages[1].top_sources, ["main/src/main.rs"]);
        assert_eq!(packages[2].name, "main2");
        assert_eq!(packages[2].top_sources, ["main2/src/main.rs"]);
    }

    #[test]
    fn package_filter_all_from_subdir_gets_everything() {
        let tmp = copy_of_testdata("workspace");
        let workspace = Workspace::open(tmp.path().join("main")).expect("Find workspace root");
        let packages = workspace.packages(&PackageFilter::All).unwrap();
        assert_eq!(
            packages.iter().map(|p| &p.name).collect_vec(),
            ["cargo_mutants_testdata_workspace_utils", "main", "main2"]
        );
    }

    #[test]
    fn auto_packages_in_workspace_subdir_finds_single_package() {
        let tmp = copy_of_testdata("workspace");
        let subdir_path = Utf8PathBuf::try_from(tmp.path().join("main")).unwrap();
        let workspace = Workspace::open(&subdir_path).expect("Find workspace root");
        let packages = workspace
            .packages(&PackageFilter::Auto(subdir_path.clone()))
            .unwrap();
        assert_eq!(packages.iter().map(|p| &p.name).collect_vec(), ["main"]);
    }

    #[test]
    fn auto_packages_in_virtual_workspace_gets_everything() {
        let tmp = copy_of_testdata("workspace");
        let workspace = Workspace::open(tmp.path()).expect("Find workspace root");
        let packages = workspace
            .packages(&PackageFilter::Auto(
                tmp.path().to_owned().try_into().unwrap(),
            ))
            .unwrap();
        assert_eq!(
            packages.iter().map(|p| &p.name).collect_vec(),
            ["cargo_mutants_testdata_workspace_utils", "main", "main2"]
        );
    }

    #[test]
    fn filter_by_single_package() {
        let tmp = copy_of_testdata("workspace");
        let workspace = Workspace::open(tmp.path().join("main")).expect("Find workspace root");
        let root_dir = workspace.root();
        assert_eq!(
            root_dir.canonicalize().unwrap(),
            tmp.path().canonicalize().unwrap()
        );
        let filter = PackageFilter::explicit(["main"]);
        let packages = workspace.packages(&filter).unwrap();
        println!("{packages:#?}");
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "main");
        assert_eq!(packages[0].top_sources, ["main/src/main.rs"]);
    }

    #[test]
    fn filter_by_multiple_packages() {
        let tmp = copy_of_testdata("workspace");
        let workspace = Workspace::open(tmp.path().join("main")).expect("Find workspace root");
        assert_eq!(
            workspace.root().canonicalize().unwrap(),
            tmp.path().canonicalize().unwrap()
        );
        let selection = PackageFilter::explicit(["main", "main2"]);
        let discovered = workspace
            .discover(&selection, &Options::default(), &Console::new())
            .unwrap();

        assert_eq!(
            discovered
                .files
                .iter()
                .map(|sf| sf.tree_relative_path.clone())
                .collect_vec(),
            ["main/src/main.rs", "main2/src/main.rs"]
        );
    }
}

// Copyright 2023-2025 Martin Pool

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
use itertools::Itertools;
use serde_json::Value;
use tracing::{debug, error, warn};

use crate::cargo::cargo_bin;
use crate::console::Console;
use crate::interrupt::check_interrupted;
use crate::options::Options;
use crate::package::{packages_from_metadata, Package, PackageSelection};
use crate::visit::{walk_tree, Discovered};
use crate::Result;

/// Which packages to mutate in a workspace?
///
/// This expresses the user's _intention_ for what to mutate, in general. It's later resolved
/// to a specific list of packages based on the workspace structure.
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
        let packages = packages_from_metadata(&metadata);
        debug!(?packages, "Found packages");
        Ok(Workspace { metadata, packages })
    }

    pub fn packages_by_name<S: AsRef<str>>(&self, names: &[S]) -> Vec<Package> {
        names
            .iter()
            .map(AsRef::as_ref)
            .sorted()
            .filter_map(|name| {
                let p = self.packages.iter().find(|p| p.name == name);
                if p.is_none() {
                    warn!("Package {name:?} not found in source tree");
                }
                p.cloned()
            })
            .collect()
    }

    /// Match a `PackageFilter` to the actual packages in this workspace, returning a list of packages.
    fn filter_packages(&self, filter: &PackageFilter) -> Result<PackageSelection> {
        match filter {
            PackageFilter::Auto(dir) => {
                let root = self.root();
                // Find the closest package directory (with a cargo manifest) to the current directory.
                let package_dir = locate_project(dir, false)?;
                assert!(package_dir.is_absolute());
                // It's not required that the members be inside the workspace directory: see
                // <https://doc.rust-lang.org/cargo/reference/workspaces.html>
                for package in &self.packages {
                    // If this package is one of the workspace members, then select this package.
                    if root.join(&package.relative_dir) == package_dir {
                        debug!(
                            package = package.name,
                            ?package_dir,
                            "Resolved auto package filter based on enclosing directory"
                        );
                        return Ok(PackageSelection::Explicit(vec![package.clone()]));
                    }
                }
                // Otherwise, we're in a virtual workspace directory, and not inside any package.
                // Use configured defaults if there are any, otherwise test all packages.
                ensure!(
                    package_dir == root,
                    "package {package_dir:?} doesn't match any child and doesn't match the workspace root {root:?}?",
                );
                let default_packages = self.default_packages();
                debug!(
                    ?default_packages,
                    "Resolved auto package filter to workspace default packages"
                );
                Ok(default_packages)
            }
            PackageFilter::All => Ok(PackageSelection::All),
            PackageFilter::Explicit(names) => {
                Ok(PackageSelection::Explicit(self.packages_by_name(names)))
            }
        }
    }

    fn expand_selection(&self, selection: PackageSelection) -> Vec<Package> {
        match selection {
            PackageSelection::All => self.packages.clone(),
            PackageSelection::Explicit(packages) => packages,
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
            &self.expand_selection(self.filter_packages(package_filter)?),
            options,
            console,
        )
    }

    /// Return the default workspace packages.
    ///
    /// Default packages can be specified in the workspace's `Cargo.toml` file;
    /// if not, all packages are included.
    fn default_packages(&self) -> PackageSelection {
        let metadata = &self.metadata;
        // `cargo_metadata::workspace_default_packages` will panic when calling Cargo older than 1.71;
        // in that case we'll just fall back to everything, for lack of a better option.
        // TODO: Use the new cargo_metadata API that doesn't panic?
        match catch_unwind(|| metadata.workspace_default_packages()) {
            Ok(default_packages) if default_packages.is_empty() => {
                debug!("manifest has no explicit default packages");
                PackageSelection::All
            }
            Ok(default_packages) => {
                let default_package_names: Vec<&str> = default_packages
                    .iter()
                    .map(|pmeta| pmeta.name.as_str())
                    .sorted() // for reproducibility
                    .collect();
                debug!(
                    ?default_package_names,
                    "Manifest defines explicit default packages"
                );
                PackageSelection::Explicit(self.packages_by_name(&default_package_names))
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
    use assert_matches::assert_matches;
    use camino::Utf8PathBuf;
    use itertools::Itertools;

    use crate::console::Console;
    use crate::options::Options;
    use crate::package::PackageSelection;
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
        let packages = workspace.filter_packages(&PackageFilter::All).unwrap();
        assert_matches!(packages, super::PackageSelection::All);
        let packages = workspace.expand_selection(packages);
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
        let packages = workspace.filter_packages(&PackageFilter::All).unwrap();
        assert_matches!(packages, super::PackageSelection::All);
        let packages = workspace.expand_selection(packages);
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
            .filter_packages(&PackageFilter::Auto(subdir_path.clone()))
            .unwrap();
        let PackageSelection::Explicit(packages) = packages else {
            panic!("Expected PackageSelection::Explicit, got {packages:?}");
        };
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "main");
    }

    #[test]
    fn auto_packages_in_virtual_workspace_gets_everything() {
        let tmp = copy_of_testdata("workspace");
        let workspace = Workspace::open(tmp.path()).expect("Find workspace root");
        let packages = workspace
            .filter_packages(&PackageFilter::Auto(
                tmp.path().to_owned().try_into().unwrap(),
            ))
            .unwrap();
        let PackageSelection::Explicit(packages) = packages else {
            panic!("Expected PackageSelection::Explicit, got {packages:?}");
        };
        assert_eq!(
            packages.iter().map(|p| &p.name).sorted().collect_vec(),
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
        let packages = workspace.filter_packages(&filter).unwrap();
        println!("{packages:#?}");
        let PackageSelection::Explicit(packages) = packages else {
            panic!("Expected PackageSelection::Explicit, got {packages:?}");
        };
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

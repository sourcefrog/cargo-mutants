// Copyright 2023-2024 Martin Pool

use std::fmt;
use std::panic::catch_unwind;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use anyhow::{anyhow, bail, ensure, Context};
use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools;
use serde_json::Value;
use tracing::{debug, debug_span, error, warn};

use crate::cargo::cargo_bin;
use crate::console::Console;
use crate::interrupt::check_interrupted;
use crate::mutate::Mutant;
use crate::options::Options;
use crate::package::Package;
use crate::source::SourceFile;
use crate::visit::{walk_tree, Discovered};
use crate::Result;

/// Which packages to mutate in a workspace?
#[derive(Debug, Clone)]
pub enum PackageFilter {
    /// Include every package in the workspace.
    All,
    /// Packages with given names, from `--package`.
    Explicit(Vec<String>),
    /// Automatic behavior when invoked from a subdirectory, as per
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
    pub fn explicit<S: ToString, I: IntoIterator<Item = S>>(names: I) -> PackageFilter {
        PackageFilter::Explicit(names.into_iter().map(|s| s.to_string()).collect_vec())
    }

    /// Translate an auto package filter to either All or Explicit.
    pub fn resolve_auto(&self, metadata: &cargo_metadata::Metadata) -> Result<PackageFilter> {
        if let PackageFilter::Auto(dir) = &self {
            let package_dir = locate_project(dir, false)?;
            assert!(package_dir.is_absolute());
            let workspace_dir = &metadata.workspace_root;
            // It's not required that the members be inside the workspace directory: see
            // <https://doc.rust-lang.org/cargo/reference/workspaces.html>
            for package in metadata.workspace_packages() {
                if package.manifest_path.parent().expect("remove Cargo.toml") == package_dir {
                    debug!("resolved auto package filter to {:?}", package.name);
                    return Ok(PackageFilter::explicit([&package.name]));
                }
            }
            // Presumably our manifest is the workspace root manifest and there is no
            // top-level package?
            ensure!(
                &package_dir == workspace_dir,
                "package {package_dir:?} doesn't match any child and doesn't match the workspace root {workspace_dir:?}?",
            );
            // `workspace_default_packages` will panic when calling Cargo older than 1.71;
            // in that case we'll just fall back to everything, for lack of a better option.
            match catch_unwind(|| metadata.workspace_default_packages()) {
                Ok(dm) if dm.is_empty() => Ok(PackageFilter::All),
                Ok(dm) => Ok(PackageFilter::explicit(
                    dm.into_iter().map(|pmeta| &pmeta.name),
                )),
                Err(err) => {
                    warn!(
                        cargo_metadata_error =
                            err.downcast::<String>().expect("panic message is a string"),
                        "workspace_default_packages is not supported; testing all packages",
                    );
                    Ok(PackageFilter::All)
                }
            }
        } else {
            Ok(self.clone())
        }
    }
}

/// A package and the top source files within it.
struct PackageTop {
    package: Arc<Package>,
    top_sources: Vec<Utf8PathBuf>,
}

/// A cargo workspace.
pub struct Workspace {
    metadata: cargo_metadata::Metadata,
}

impl fmt::Debug for Workspace {
    #[mutants::skip]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Workspace")
            .field("root", &self.root().to_string())
            // .field("metadata", &self.metadata)
            .finish()
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
            .with_context(|| format!("Failed to run cargo metadata on {:?}", manifest_path))?;
        debug!(workspace_root = ?metadata.workspace_root, "Found workspace root");
        Ok(Workspace { metadata })
    }

    /// Find packages to mutate, subject to some filtering.
    #[allow(dead_code)]
    pub fn packages(&self, package_filter: &PackageFilter) -> Result<Vec<Arc<Package>>> {
        Ok(self
            .package_tops(package_filter)?
            .into_iter()
            .map(|pt| pt.package)
            .collect())
    }

    /// Find all the packages and their top source files.
    fn package_tops(&self, package_filter: &PackageFilter) -> Result<Vec<PackageTop>> {
        let mut tops = Vec::new();
        let package_filter = package_filter.resolve_auto(&self.metadata)?;
        for package_metadata in self
            .metadata
            .workspace_packages()
            .into_iter()
            .sorted_by_key(|p| &p.name)
        {
            check_interrupted()?;
            let name = &package_metadata.name;
            let _span = debug_span!("package", %name).entered();
            if let PackageFilter::Explicit(ref include_names) = package_filter {
                if !include_names.contains(name) {
                    continue;
                }
            }
            let manifest_path = &package_metadata.manifest_path;
            debug!(%manifest_path, "walk package");
            let relative_manifest_path = manifest_path
                .strip_prefix(self.root())
                .map_err(|_| {
                    anyhow!(
                        "manifest path {manifest_path:?} for package {name:?} is not \
                    within the detected source root path {dir:?}",
                        dir = self.root(),
                    )
                })?
                .to_owned();
            let package = Arc::new(Package {
                name: package_metadata.name.clone(),
                relative_manifest_path,
            });
            tops.push(PackageTop {
                package,
                top_sources: direct_package_sources(self.root(), package_metadata)?,
            });
        }
        if let PackageFilter::Explicit(ref names) = package_filter {
            for wanted in names {
                if !tops.iter().any(|found| found.package.name == *wanted) {
                    warn!("package {wanted:?} not found in source tree");
                }
            }
        }
        Ok(tops)
    }

    /// Find all the top source files for selected packages.
    fn top_sources(&self, package_filter: &PackageFilter) -> Result<Vec<SourceFile>> {
        let mut sources = Vec::new();
        for PackageTop {
            package,
            top_sources,
        } in self.package_tops(package_filter)?
        {
            for source_path in top_sources {
                sources.extend(SourceFile::new(
                    self.root(),
                    source_path.to_owned(),
                    &package,
                    true,
                )?);
            }
        }
        Ok(sources)
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
            &self.top_sources(package_filter)?,
            options,
            console,
        )
    }

    /// Return all mutants generated from this workspace.
    #[allow(dead_code)] // called from tests, for now
    pub fn mutants(
        &self,
        package_filter: &PackageFilter,
        options: &Options,
        console: &Console,
    ) -> Result<Vec<Mutant>> {
        Ok(self.discover(package_filter, options, console)?.mutants)
    }
}

/// Find all the files that are named in the `path` of targets in a Cargo manifest that should be tested.
///
/// These are the starting points for discovering source files.
fn direct_package_sources(
    workspace_root: &Utf8Path,
    package_metadata: &cargo_metadata::Package,
) -> Result<Vec<Utf8PathBuf>> {
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
    Ok(found)
}

fn should_mutate_target(target: &cargo_metadata::Target) -> bool {
    for kind in target.kind.iter() {
        if kind == "bin" || kind == "proc-macro" || kind.ends_with("lib") {
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
    use camino::{Utf8Path, Utf8PathBuf};
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
        assert_eq!(
            workspace
                .packages(&PackageFilter::All)
                .unwrap()
                .iter()
                .map(|p| p.name.clone())
                .collect_vec(),
            ["cargo_mutants_testdata_workspace_utils", "main", "main2"]
        );
        assert_eq!(
            workspace
                .top_sources(&PackageFilter::All)
                .unwrap()
                .iter()
                .map(|sf| sf.tree_relative_path.clone())
                .collect_vec(),
            // ordered by package name
            ["utils/src/lib.rs", "main/src/main.rs", "main2/src/main.rs"]
        );
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
            .packages(&PackageFilter::Auto(subdir_path.to_owned()))
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
        assert_eq!(
            workspace
                .packages(&filter)
                .unwrap()
                .iter()
                .map(|p| p.name.clone())
                .collect_vec(),
            ["main"]
        );
        let top_sources = workspace.top_sources(&filter).unwrap();
        println!("{top_sources:#?}");
        assert_eq!(
            top_sources
                .iter()
                .map(|sf| sf.tree_relative_path.clone())
                .collect_vec(),
            [Utf8Path::new("main/src/main.rs")]
        );
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

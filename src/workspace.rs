// Copyright 2023 Martin Pool

use std::fmt;
use std::sync::Arc;

use anyhow::{anyhow, ensure, Context};
use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools;
use serde_json::Value;
use tracing::{debug, debug_span, warn};

use crate::cargo::cargo_bin;
use crate::console::Console;
use crate::interrupt::check_interrupted;
use crate::mutate::Mutant;
use crate::options::Options;
use crate::package::Package;
use crate::process::get_command_output;
use crate::source::SourceFile;
use crate::visit::{walk_tree, Discovered};
use crate::Result;

pub struct Workspace {
    pub dir: Utf8PathBuf,
    metadata: cargo_metadata::Metadata,
}

impl fmt::Debug for Workspace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Workspace")
            .field("dir", &self.dir)
            // .field("metadata", &self.metadata)
            .finish()
    }
}

pub enum PackageFilter {
    All,
    Explicit(Vec<String>),
    Auto(Utf8PathBuf),
}

impl PackageFilter {
    pub fn explicit<S: ToString, I: IntoIterator<Item = S>>(names: I) -> PackageFilter {
        PackageFilter::Explicit(names.into_iter().map(|s| s.to_string()).collect_vec())
    }
}

impl Workspace {
    pub fn open(start_dir: &Utf8Path) -> Result<Self> {
        let dir = find_workspace(start_dir)?;
        let cargo_toml_path = dir.join("Cargo.toml");
        debug!(?cargo_toml_path, ?dir, "Find root files");
        check_interrupted()?;
        let metadata = cargo_metadata::MetadataCommand::new()
            .manifest_path(&cargo_toml_path)
            .exec()
            .context("run cargo metadata")?;
        Ok(Workspace { dir, metadata })
    }

    /// Find packages to mutate, subject to some filtering.
    pub fn packages(&self, package_filter: &PackageFilter) -> Result<Vec<Arc<Package>>> {
        let mut packages = Vec::new();
        for package_metadata in filter_package_metadata(&self.metadata, package_filter)
            .into_iter()
            .sorted_by_key(|p| &p.name)
        {
            check_interrupted()?;
            let name = &package_metadata.name;
            let _span = debug_span!("package", %name).entered();
            let manifest_path = &package_metadata.manifest_path;
            debug!(%manifest_path, "walk package");
            let relative_manifest_path = manifest_path
                .strip_prefix(&self.dir)
                .map_err(|_| {
                    anyhow!(
                        "manifest path {manifest_path:?} for package {name:?} is not \
                    within the detected source root path {dir:?}",
                        dir = self.dir
                    )
                })?
                .to_owned();
            let package = Arc::new(Package {
                name: package_metadata.name.clone(),
                relative_manifest_path,
            });
            packages.push(package);
        }
        if let PackageFilter::Explicit(names) = package_filter {
            for wanted in names {
                if !packages.iter().any(|found| found.name == *wanted) {
                    warn!("package {wanted:?} not found in source tree");
                }
            }
        }
        Ok(packages)
    }

    /// Return the top source files (like `src/lib.rs`) for a named package.
    fn top_package_sources(&self, package_name: &str) -> Result<Vec<Utf8PathBuf>> {
        if let Some(package_metadata) = self
            .metadata
            .workspace_packages()
            .iter()
            .find(|p| p.name == package_name)
        {
            direct_package_sources(&self.dir, package_metadata)
        } else {
            Err(anyhow!(
                "package {package_name:?} not found in workspace metadata"
            ))
        }
    }

    /// Find all the top source files for selected packages.
    pub fn top_sources(&self, package_filter: &PackageFilter) -> Result<Vec<Arc<SourceFile>>> {
        let mut sources = Vec::new();
        for package in self.packages(package_filter)? {
            for source_path in self.top_package_sources(&package.name)? {
                sources.push(Arc::new(SourceFile::new(
                    &self.dir,
                    source_path.to_owned(),
                    &package,
                )?));
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
            &self.dir,
            &self.top_sources(package_filter)?,
            options,
            console,
        )
    }

    pub fn mutants(
        &self,
        package_filter: &PackageFilter,
        options: &Options,
        console: &Console,
    ) -> Result<Vec<Mutant>> {
        Ok(self.discover(package_filter, options, console)?.mutants)
    }
}

fn filter_package_metadata<'m>(
    metadata: &'m cargo_metadata::Metadata,
    package_filter: &PackageFilter,
) -> Vec<&'m cargo_metadata::Package> {
    metadata
        .workspace_packages()
        .iter()
        .filter(move |pmeta| match package_filter {
            PackageFilter::All => true,
            PackageFilter::Explicit(include_names) => include_names.contains(&pmeta.name),
            PackageFilter::Auto(..) => todo!(),
        })
        .sorted_by_key(|pm| &pm.name)
        .copied()
        .collect()
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
    target.kind.iter().any(|k| k.ends_with("lib") || k == "bin")
}

/// Return the path of the workspace directory enclosing a given directory.
fn find_workspace(path: &Utf8Path) -> Result<Utf8PathBuf> {
    ensure!(path.is_dir(), "{path:?} is not a directory");
    let cargo_bin = cargo_bin(); // needed for lifetime
    let argv: Vec<&str> = vec![&cargo_bin, "locate-project", "--workspace"];
    let stdout = get_command_output(&argv, path)
        .with_context(|| format!("run cargo locate-project in {path:?}"))?;
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
    use std::ffi::OsStr;

    use camino::Utf8Path;
    use itertools::Itertools;

    use crate::console::Console;
    use crate::options::Options;
    use crate::workspace::PackageFilter;

    use super::Workspace;

    #[test]
    fn error_opening_outside_of_crate() {
        Workspace::open(&Utf8Path::new("/")).unwrap_err();
    }

    #[test]
    fn open_subdirectory_of_crate_opens_the_crate() {
        let workspace = Workspace::open(Utf8Path::new("testdata/tree/factorial/src"))
            .expect("open source tree from subdirectory");
        let root = &workspace.dir;
        assert!(root.is_dir());
        assert!(root.join("Cargo.toml").is_file());
        assert!(root.join("src/bin/factorial.rs").is_file());
        assert_eq!(root.file_name().unwrap(), OsStr::new("factorial"));
    }

    #[test]
    fn find_root_from_subdirectory_of_workspace_finds_the_workspace_root() {
        let root = Workspace::open(Utf8Path::new("testdata/tree/workspace/main"))
            .expect("Find root from within workspace/main")
            .dir;
        assert_eq!(root.file_name(), Some("workspace"), "Wrong root: {root:?}");
    }

    #[test]
    fn find_top_source_files_from_subdirectory_of_workspace() {
        let workspace = Workspace::open(Utf8Path::new("testdata/tree/workspace/main"))
            .expect("Find workspace root");
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
    fn filter_by_single_package() {
        let workspace = Workspace::open(Utf8Path::new("testdata/tree/workspace/main"))
            .expect("Find workspace root");
        let root_dir = &workspace.dir;
        assert_eq!(
            root_dir.file_name(),
            Some("workspace"),
            "found the workspace root"
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
        let workspace = Workspace::open(Utf8Path::new("testdata/tree/workspace/main")).unwrap();
        assert_eq!(
            workspace.dir.file_name(),
            Some("workspace"),
            "found the workspace root"
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

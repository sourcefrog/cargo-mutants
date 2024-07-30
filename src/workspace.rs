// Copyright 2023 Martin Pool

use std::fmt;
use std::panic::catch_unwind;
use std::sync::Arc;

use anyhow::{anyhow, ensure, Context};
use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools;
use serde_json::Value;
use tracing::{debug, debug_span, warn};

use crate::cargo::cargo_bin;
use crate::console::Console;
use crate::find_files::find_source_files;
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
    #[mutants::skip]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Workspace")
            .field("dir", &self.dir)
            // .field("metadata", &self.metadata)
            .finish()
    }
}

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

impl Workspace {
    /// Open the workspace containing a given directory.
    pub fn open<P: AsRef<Utf8Path>>(start_dir: P) -> Result<Self> {
        let dir = locate_project(start_dir.as_ref(), true)?;
        let manifest_path = dir.join("Cargo.toml");
        debug!(?manifest_path, ?dir, "Find root files");
        check_interrupted()?;
        let metadata = cargo_metadata::MetadataCommand::new()
            .no_deps()
            .manifest_path(&manifest_path)
            .exec()
            .with_context(|| format!("Failed to run cargo metadata on {:?}", manifest_path))?;
        debug!(workspace_root = ?metadata.workspace_root, "Found workspace root");
        Ok(Workspace { dir, metadata })
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
            tops.push(PackageTop {
                package,
                top_sources: direct_package_sources(&self.dir, package_metadata)?,
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
                    &self.dir,
                    source_path.to_owned(),
                    &package,
                    true,
                )?);
            }
        }
        Ok(sources)
    }

    /// Find all the source files in the workspace.
    fn source_files(
        &self,
        package_filter: &PackageFilter,
        options: &Options,
        console: &Console,
    ) -> Result<Vec<SourceFile>> {
        let top_sources = self.top_sources(package_filter)?;
        find_source_files(&self.dir, &top_sources, options, console)
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
    target.kind.iter().any(|k| k.ends_with("lib") || k == "bin")
}

/// Return the path of the workspace or package directory enclosing a given directory.
fn locate_project(path: &Utf8Path, workspace: bool) -> Result<Utf8PathBuf> {
    ensure!(path.is_dir(), "{path:?} is not a directory");
    let cargo_bin = cargo_bin(); // needed for lifetime
    let mut argv: Vec<&str> = vec![&cargo_bin, "locate-project"];
    if workspace {
        argv.push("--workspace");
    }
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
        Workspace::open("/").unwrap_err();
    }

    #[test]
    fn open_subdirectory_of_crate_opens_the_crate() {
        let workspace =
            Workspace::open("testdata/factorial/src").expect("open source tree from subdirectory");
        let root = &workspace.dir;
        assert!(root.is_dir());
        assert!(root.join("Cargo.toml").is_file());
        assert!(root.join("src/bin/factorial.rs").is_file());
        assert_eq!(root.file_name().unwrap(), OsStr::new("factorial"));
    }

    #[test]
    fn find_root_from_subdirectory_of_workspace_finds_the_workspace_root() {
        let root = Workspace::open("testdata/workspace/main")
            .expect("Find root from within workspace/main")
            .dir;
        assert_eq!(root.file_name(), Some("workspace"), "Wrong root: {root:?}");
    }

    #[test]
    fn find_top_source_files_from_subdirectory_of_workspace() {
        let workspace = Workspace::open("testdata/workspace/main").expect("Find workspace root");
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
    fn find_files_in_single_file_tree() {
        let tree_path = Utf8Path::new("testdata/small_well_tested");
        let workspace = Workspace::open(tree_path).unwrap();
        let options = Options::default();
        let source_files = workspace
            .source_files(&PackageFilter::All, &options, &Console::new())
            .unwrap();
        assert_eq!(source_files.len(), 1);
        assert_eq!(source_files[0].tree_relative_path, "src/lib.rs");
    }

    #[test]
    fn find_files_in_nested_mod_tree() {
        let tree_path = Utf8Path::new("testdata/nested_mod");
        let workspace = Workspace::open(tree_path).unwrap();
        let options = Options::default();
        let source_files = workspace
            .source_files(&PackageFilter::All, &options, &Console::new())
            .unwrap();
        assert_eq!(
            source_files
                .iter()
                .map(|sf| &sf.tree_relative_path)
                .sorted()
                .collect_vec(),
            [
                "src/block_in_lib/a/b/c_file/d/e/f_file.rs",
                "src/block_in_lib/a/b/c_file.rs",
                "src/block_in_main/a/b/c_file/d/e/f_file.rs",
                "src/block_in_main/a/b/c_file.rs",
                "src/file_in_lib/a/b/c_file/d/e/f_file.rs",
                "src/file_in_lib/a/b/c_file.rs",
                "src/file_in_lib.rs",
                "src/file_in_main/a/b/c_file/d/e/f_file.rs",
                "src/file_in_main/a/b/c_file.rs",
                "src/file_in_main.rs",
                "src/lib.rs",
                "src/main.rs",
                "src/paths_in_lib/../upward_traversal_file_for_lib.rs",
                "src/paths_in_lib/a/b/inline/other.rs",
                "src/paths_in_lib/a/b.rs",
                "src/paths_in_lib/a/foo.rs",
                "src/paths_in_lib/a_mod_file/foo.rs",
                "src/paths_in_lib/a_mod_file/inline/other.rs",
                "src/paths_in_lib/a_mod_file/mod.rs",
                "src/paths_in_lib/thread_files/tls.rs",
                "src/paths_in_lib/thread_files_inner_attr/tls.rs",
                "src/paths_in_lib/upward_traversal.rs",
                "src/paths_in_main/a/b/inline/other.rs",
                "src/paths_in_main/a/b.rs",
                "src/paths_in_main/a/foo.rs",
                "src/paths_in_main/a_mod_file/foo.rs",
                "src/paths_in_main/a_mod_file/inline/other.rs",
                "src/paths_in_main/a_mod_file/mod.rs",
                "src/paths_in_main/thread_files/tls.rs",
                "src/paths_in_main/thread_files_inner_attr/tls.rs",
                "src/toplevel_file_in_lib.rs",
                "src/toplevel_file_in_main.rs"
            ]
        );
    }

    #[test]
    fn package_filter_all_from_subdir_gets_everything() {
        let subdir_path = Utf8Path::new("testdata/workspace/main");
        let workspace = Workspace::open(subdir_path).expect("Find workspace root");
        let packages = workspace.packages(&PackageFilter::All).unwrap();
        assert_eq!(
            packages.iter().map(|p| &p.name).collect_vec(),
            ["cargo_mutants_testdata_workspace_utils", "main", "main2"]
        );
    }

    #[test]
    fn auto_packages_in_workspace_subdir_finds_single_package() {
        let subdir_path = Utf8Path::new("testdata/workspace/main");
        let workspace = Workspace::open(subdir_path).expect("Find workspace root");
        let packages = workspace
            .packages(&PackageFilter::Auto(subdir_path.to_owned()))
            .unwrap();
        assert_eq!(packages.iter().map(|p| &p.name).collect_vec(), ["main"]);
    }

    #[test]
    fn auto_packages_in_virtual_workspace_gets_everything() {
        let path = Utf8Path::new("testdata/workspace");
        let workspace = Workspace::open(path).expect("Find workspace root");
        let packages = workspace
            .packages(&PackageFilter::Auto(path.to_owned()))
            .unwrap();
        assert_eq!(
            packages.iter().map(|p| &p.name).collect_vec(),
            ["cargo_mutants_testdata_workspace_utils", "main", "main2"]
        );
    }

    #[test]
    fn filter_by_single_package() {
        let workspace = Workspace::open("testdata/workspace/main").expect("Find workspace root");
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
        let workspace = Workspace::open("testdata/workspace/main").unwrap();
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

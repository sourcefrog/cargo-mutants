// Copyright 2021-2024 Martin Pool

//! A directory containing mutated source to run cargo builds and tests.

use std::fmt::{self, Debug};
use std::fs::write;

use tempfile::TempDir;
use tracing::info;

use crate::copy_tree::copy_tree;
use crate::manifest::fix_cargo_config;
use crate::*;

/// A directory containing source, that can be mutated, built, and tested.
///
/// Depending on how its constructed, this might be a copy in a tempdir
/// or the original source directory.
pub struct BuildDir {
    /// The path of the root of the build directory.
    path: Utf8PathBuf,
    /// Holds a reference to the temporary directory, so that it will be deleted when this
    /// object is dropped. If None, there's nothing to clean up.
    #[allow(dead_code)]
    temp_dir: Option<TempDir>,
}

impl Debug for BuildDir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BuildDir")
            .field("path", &self.path)
            .finish()
    }
}

impl BuildDir {
    /// Make a new build dir, copying from a source directory, subject to exclusions.
    pub fn copy_from(
        source: &Utf8Path,
        gitignore: bool,
        leak_temp_dir: bool,
        console: &Console,
    ) -> Result<BuildDir> {
        let name_base = format!("cargo-mutants-{}-", source.file_name().unwrap_or("unnamed"));
        let source_abs = source
            .canonicalize_utf8()
            .context("canonicalize source path")?;
        let temp_dir = copy_tree(source, &name_base, gitignore, console)?;
        let path: Utf8PathBuf = temp_dir
            .path()
            .to_owned()
            .try_into()
            .context("tempdir path to UTF-8")?;
        fix_manifest(&path.join("Cargo.toml"), &source_abs)?;
        fix_cargo_config(&path, &source_abs)?;
        let temp_dir = if leak_temp_dir {
            let _ = temp_dir.into_path();
            info!(?path, "Build directory will be leaked for inspection");
            None
        } else {
            Some(temp_dir)
        };
        let build_dir = BuildDir { temp_dir, path };
        Ok(build_dir)
    }

    /// Make a build dir that works in-place on the source directory.
    pub fn in_place(source_path: &Utf8Path) -> Result<BuildDir> {
        Ok(BuildDir {
            temp_dir: None,
            path: source_path
                .canonicalize_utf8()
                .context("canonicalize source path")?
                .to_owned(),
        })
    }

    pub fn path(&self) -> &Utf8Path {
        self.path.as_path()
    }

    pub fn overwrite_file(&self, relative_path: &Utf8Path, code: &str) -> Result<()> {
        let full_path = self.path.join(relative_path);
        // for safety, don't follow symlinks
        ensure!(full_path.is_file(), "{full_path:?} is not a file");
        write(&full_path, code.as_bytes())
            .with_context(|| format!("failed to write code to {full_path:?}"))
    }
}

#[cfg(test)]
mod test {
    use test_util::copy_of_testdata;

    use super::*;

    #[test]
    fn build_dir_copy_from() {
        let tmp = copy_of_testdata("factorial");
        let workspace = Workspace::open(tmp.path()).unwrap();
        let build_dir =
            BuildDir::copy_from(workspace.root(), true, false, &Console::new()).unwrap();
        let debug_form = format!("{build_dir:?}");
        println!("debug form is {debug_form:?}");
        assert!(debug_form.starts_with("BuildDir { path: "));
        assert!(build_dir.path().is_dir());
        assert!(build_dir.path().join("Cargo.toml").is_file());
        assert!(build_dir.path().join("src").is_dir());
    }

    #[test]
    fn build_dir_in_place() -> Result<()> {
        let tmp = copy_of_testdata("factorial");
        let workspace = Workspace::open(tmp.path())?;
        let build_dir = BuildDir::in_place(workspace.root())?;
        // On Windows e.g. the paths might not have the same form, but they
        // should point to the same place.
        assert_eq!(
            build_dir.path().canonicalize_utf8()?,
            workspace.root().canonicalize_utf8()?
        );
        Ok(())
    }
}

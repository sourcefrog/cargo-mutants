// Copyright 2021, 2022 Martin Pool

//! A temporary directory containing mutated source to run cargo builds and tests.

use std::convert::TryInto;
use std::fmt;

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use tempfile::TempDir;
use tracing::{debug, error, trace};

use crate::manifest::fix_cargo_config;
use crate::*;

/// Filenames excluded from being copied with the source.
const SOURCE_EXCLUDE: &[&str] = &[
    ".git",
    ".hg",
    ".bzr",
    ".svn",
    "_darcs",
    ".pijul",
    "mutants.out",
    "mutants.out.old",
    "target",
];

/// A temporary directory initialized with a copy of the source, where mutations can be tested.
pub struct BuildDir {
    /// The path of the root of the temporary directory.
    path: Utf8PathBuf,
    /// A prefix for tempdir names, based on the name of the source directory.
    name_base: String,
    /// Holds a reference to the temporary directory, so that it will be deleted when this
    /// object is dropped.
    #[allow(dead_code)]
    temp_dir: TempDir,
}

impl BuildDir {
    /// Make a new build dir, copying from a source directory.
    ///
    /// [SOURCE_EXCLUDE] is excluded.
    pub fn new(source: &dyn SourceTree, console: &Console) -> Result<BuildDir> {
        let name_base = format!("cargo-mutants-{}-", source.path().file_name().unwrap_or(""));
        let source_abs = source
            .path()
            .canonicalize_utf8()
            .expect("canonicalize source path");
        let temp_dir = copy_tree(source.path(), &name_base, SOURCE_EXCLUDE, console)?;
        let path: Utf8PathBuf = temp_dir.path().to_owned().try_into().unwrap();
        fix_manifest(&path.join("Cargo.toml"), &source_abs)?;
        fix_cargo_config(&path, &source_abs)?;
        let build_dir = BuildDir {
            temp_dir,
            name_base,
            path,
        };
        Ok(build_dir)
    }

    pub fn path(&self) -> &Utf8Path {
        self.path.as_path()
    }

    /// Make a copy of this build dir, including its target directory.
    #[allow(dead_code)]
    pub fn copy(&self, console: &Console) -> Result<BuildDir> {
        let temp_dir = copy_tree(&self.path, &self.name_base, &[], console)?;
        Ok(BuildDir {
            path: temp_dir.path().to_owned().try_into().unwrap(),
            temp_dir,
            name_base: self.name_base.clone(),
        })
    }
}

impl fmt::Debug for BuildDir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BuildDir")
            .field("path", &self.path)
            .finish()
    }
}

fn copy_tree(
    from_path: &Utf8Path,
    name_base: &str,
    exclude: &[&str],
    console: &Console,
) -> Result<TempDir> {
    let temp_dir = tempfile::Builder::new()
        .prefix(&name_base)
        .suffix(".tmp")
        .tempdir()
        .context("create temp dir")?;
    console.start_copy();
    let copy_options = cp_r::CopyOptions::new()
        .after_entry_copied(|path, _ft, stats| {
            console.copy_progress(stats.file_bytes);
            check_interrupted().map_err(|_| cp_r::Error::new(cp_r::ErrorKind::Interrupted, path))
        })
        .filter(|path, _dir_entry| {
            let excluded = exclude.iter().any(|ex| path.ends_with(ex));
            if excluded {
                trace!("Skip {path:?}");
            } else {
                trace!("Copy {path:?}");
            }
            Ok(!excluded)
        });
    match copy_options
        .copy_tree(from_path, temp_dir.path())
        .context("copy tree")
    {
        Ok(stats) => {
            debug!(files = stats.files, file_bytes = stats.file_bytes,);
        }
        Err(err) => {
            error!(
                "error copying {} to {}: {:?}",
                &from_path.to_slash_path(),
                &temp_dir.path().to_slash_lossy(),
                err
            );
            return Err(err);
        }
    }
    console.finish_copy();
    Ok(temp_dir)
}

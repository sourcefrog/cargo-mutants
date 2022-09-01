// Copyright 2021, 2022 Martin Pool

//! A temporary directory containing mutated source to run cargo builds and tests.

use std::convert::TryInto;
use std::path::Path;

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use tempfile::TempDir;
use tracing::error;

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
];

/// A temporary directory initialized with a copy of the source, where mutations can be tested.
#[derive(Debug)]
pub struct BuildDir {
    /// The path of the root of the temporary directory.
    path: Utf8PathBuf,
    /// Holds a reference to the temporary directory, so that it will be deleted when this
    /// object is dropped.
    _temp_dir: TempDir,
}

impl BuildDir {
    /// Make a new build dir, copying from a source directory.
    pub fn new(source: &SourceTree, options: &Options) -> Result<BuildDir> {
        let name_tail = source.path().file_name().unwrap_or("");
        let temp_dir = tempfile::Builder::new()
            .prefix(&format!("cargo-mutants-{}-", name_tail))
            .suffix(".tmp")
            .tempdir()
            .context("create temp dir")?;
        let build_path: Utf8PathBuf = temp_dir.path().to_owned().try_into().unwrap();
        let copy_target = options.copy_target;
        let name = if copy_target {
            "Copy source and build products to scratch directory"
        } else {
            "Copy source to scratch directory"
        };
        let view = nutmeg::View::new(
            console::CopyModel::new(name, options),
            console::nutmeg_options(),
        );
        let target_path = Path::new("target");
        let copy_options = cp_r::CopyOptions::new()
            .after_entry_copied(|path, _ft, stats| {
                view.update(|model| model.bytes_copied(stats.file_bytes));
                check_interrupted()
                    .map_err(|_| cp_r::Error::new(cp_r::ErrorKind::Interrupted, path))
            })
            .filter(|path, dir_entry| {
                Ok(!SOURCE_EXCLUDE.iter().any(|ex| path.ends_with(ex))
                    && (copy_target
                        || !(dir_entry.file_type().unwrap().is_dir() && path == target_path)))
            });
        match copy_options
            .copy_tree(source.path(), &temp_dir.path())
            .context("copy source tree to lab directory")
        {
            Ok(stats) => {
                view.update(|model| model.succeed(stats.file_bytes));
                view.finish();
            }
            Err(err) => {
                view.finish();
                error!(
                    "error copying source tree {} to {}: {:?}",
                    &source.path().to_slash_path(),
                    &temp_dir.path().to_slash_lossy(),
                    err
                );
                return Err(err);
            }
        }
        let source_abs = source
            .path()
            .canonicalize_utf8()
            .expect("canonicalize source path");
        fix_manifest(&build_path.join("Cargo.toml"), &source_abs)?;
        fix_cargo_config(&build_path, &source_abs)?;
        let build_dir = BuildDir {
            _temp_dir: temp_dir,
            path: build_path,
        };
        Ok(build_dir)
    }

    pub fn path(&self) -> &Utf8Path {
        self.path.as_path()
    }
}

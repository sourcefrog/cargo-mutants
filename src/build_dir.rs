//! A temporary directory containing mutated source to run cargo builds and tests.

use std::convert::TryInto;
use std::path::Path;

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use tempfile::TempDir;

use crate::console::CopyActivity;
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
    path: Utf8PathBuf,
    _temp_dir: TempDir,
}

impl BuildDir {
    /// Make a new build dir, copying from a source directory.
    pub fn new(source: &SourceTree, options: &Options) -> Result<BuildDir> {
        let temp_dir = tempfile::Builder::new()
            .prefix(&format!(
                "cargo-mutants-{}-",
                source.path().file_name().unwrap_or_default()
            ))
            .suffix(".tmp")
            .tempdir()
            .context("create temp dir")?;
        let temp_dir_path = temp_dir.path().to_owned().try_into().unwrap();
        let copy_target = options.copy_target;
        let name = if copy_target {
            "Copy source and build products to scratch directory"
        } else {
            "Copy source to scratch directory"
        };
        let mut activity = CopyActivity::new(name, options.clone());
        let target_path = Path::new("target");
        match cp_r::CopyOptions::new()
            .after_entry_copied(|path, _ft, stats| {
                activity.bytes_copied(stats.file_bytes);
                check_interrupted()
                    .map_err(|_| cp_r::Error::new(cp_r::ErrorKind::Interrupted, path))
            })
            .filter(|path, dir_entry| {
                Ok(!SOURCE_EXCLUDE.iter().any(|ex| path.ends_with(ex))
                    && (copy_target
                        || !(dir_entry.file_type().unwrap().is_dir() && path == target_path)))
            })
            .copy_tree(source.path(), &temp_dir.path())
            .context("copy source tree to lab directory")
        {
            Ok(stats) => activity.succeed(stats.file_bytes),
            Err(err) => {
                activity.fail();
                eprintln!(
                    "error copying source tree {} to {}: {:?}",
                    &source.path().to_slash_path(),
                    &temp_dir.path().to_slash_lossy(),
                    err
                );
                return Err(err);
            }
        }
        Ok(BuildDir {
            _temp_dir: temp_dir,
            path: temp_dir_path,
        })
    }

    pub fn path(&self) -> &Utf8Path {
        self.path.as_path()
    }
}

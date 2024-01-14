// Copyright 2023 Martin Pool

//! Copy a source tree, with some exclusions, to a new temporary directory.

use std::convert::TryInto;
use std::fs::FileType;

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use ignore::WalkBuilder;
use path_slash::PathExt;
use tempfile::TempDir;
use tracing::{debug, warn};

use crate::check_interrupted;
use crate::Console;
use crate::Result;

/// Filenames excluded from being copied with the source.
static SOURCE_EXCLUDE: &[&str] = &[
    ".git",
    ".hg",
    ".bzr",
    ".svn",
    "_darcs",
    ".pijul",
    "mutants.out",
    "mutants.out.old",
];

/// Copy a source tree, with some exclusions, to a new temporary directory.
///
/// If `git` is true, ignore files that are excluded by all the various `.gitignore`
/// files.
///
/// Regardless, anything matching [SOURCE_EXCLUDE] is excluded.
pub fn copy_tree(
    from_path: &Utf8Path,
    name_base: &str,
    gitignore: bool,
    console: &Console,
) -> Result<TempDir> {
    console.start_copy();
    let mut total_bytes = 0;
    let mut total_files = 0;
    let temp_dir = tempfile::Builder::new()
        .prefix(name_base)
        .suffix(".tmp")
        .tempdir()
        .context("create temp dir")?;
    for entry in WalkBuilder::new(from_path)
        .standard_filters(gitignore)
        .hidden(false)
        .require_git(false)
        .filter_entry(|entry| {
            !SOURCE_EXCLUDE.contains(&entry.file_name().to_string_lossy().as_ref())
        })
        .build()
    {
        check_interrupted()?;
        let entry = entry?;
        let relative_path = entry
            .path()
            .strip_prefix(from_path)
            .expect("entry path is in from_path");
        let dest_path: Utf8PathBuf = temp_dir
            .path()
            .join(relative_path)
            .try_into()
            .context("Convert path to UTF-8")?;
        let ft = entry
            .file_type()
            .with_context(|| format!("Expected file to have a file type: {:?}", entry.path()))?;
        if ft.is_file() {
            let bytes_copied = std::fs::copy(entry.path(), &dest_path).with_context(|| {
                format!(
                    "Failed to copy {:?} to {dest_path:?}",
                    entry.path().to_slash_lossy(),
                )
            })?;
            total_bytes += bytes_copied;
            total_files += 1;
            console.copy_progress(total_bytes);
        } else if ft.is_dir() {
            std::fs::create_dir_all(&dest_path)
                .with_context(|| format!("Failed to create directory {dest_path:?}"))?;
        } else if ft.is_symlink() {
            copy_symlink(
                ft,
                entry
                    .path()
                    .try_into()
                    .context("Convert filename to UTF-8")?,
                &dest_path,
            )?;
        } else {
            warn!("Unexpected file type: {:?}", entry.path());
        }
    }
    console.finish_copy();
    debug!(?total_bytes, ?total_files, "Copied source tree");
    Ok(temp_dir)
}

#[cfg(unix)]
fn copy_symlink(_ft: FileType, src_path: &Utf8Path, dest_path: &Utf8Path) -> Result<()> {
    let link_target = std::fs::read_link(src_path)
        .with_context(|| format!("Failed to read link {src_path:?}"))?;
    std::os::unix::fs::symlink(link_target, dest_path)
        .with_context(|| format!("Failed to create symlink {dest_path:?}",))?;
    Ok(())
}

#[cfg(windows)]
#[mutants::skip] // Mutant tests run on Linux
fn copy_symlink(ft: FileType, src_path: &Utf8Path, dest_path: &Utf8Path) -> Result<()> {
    use std::os::windows::fs::FileTypeExt;
    let link_target =
        std::fs::read_link(src_path).with_context(|| format!("read link {src_path:?}"))?;
    if ft.is_symlink_dir() {
        std::os::windows::fs::symlink_dir(link_target, dest_path)
            .with_context(|| format!("create symlink {dest_path:?}"))?;
    } else if ft.is_symlink_file() {
        std::os::windows::fs::symlink_file(link_target, dest_path)
            .with_context(|| format!("create symlink {dest_path:?}"))?;
    } else {
        anyhow::bail!("Unknown symlink type: {:?}", ft);
    }
    Ok(())
}

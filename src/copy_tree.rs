// Copyright 2023 - 2024 Martin Pool

//! Copy a source tree, with some exclusions, to a new temporary directory.

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use ignore::WalkBuilder;
use path_slash::PathExt;
use tempfile::TempDir;
use tracing::{debug, warn};

use crate::{check_interrupted, Console, Result};

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix::copy_symlink;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows::copy_symlink;

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
/// Regardless, anything matching [`SOURCE_EXCLUDE`] is excluded.
pub fn copy_tree(
    from_path: &Utf8Path,
    name_base: &str,
    gitignore: bool,
    console: &Console,
) -> Result<TempDir> {
    let mut total_bytes = 0;
    let mut total_files = 0;
    let temp_dir = tempfile::Builder::new()
        .prefix(name_base)
        .suffix(".tmp")
        .tempdir()
        .context("create temp dir")?;
    let dest = temp_dir
        .path()
        .try_into()
        .context("Convert path to UTF-8")?;
    console.start_copy(dest);
    let mut walk_builder = WalkBuilder::new(from_path);
    walk_builder
        .standard_filters(gitignore)
        .hidden(false) // copy hidden files
        .ignore(false) // don't use .ignore
        .require_git(true) // stop at git root; only read gitignore files inside git trees
        .filter_entry(|entry| {
            !SOURCE_EXCLUDE.contains(&entry.file_name().to_string_lossy().as_ref())
        });
    debug!(?walk_builder);
    for entry in walk_builder.build() {
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
            console.copy_progress(dest, total_bytes);
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
    console.finish_copy(dest);
    debug!(?total_bytes, ?total_files, temp_dir = ?temp_dir.path(), "Copied source tree");
    Ok(temp_dir)
}

#[cfg(test)]
mod test {
    use std::fs::{create_dir, write};

    use camino::Utf8PathBuf;
    use tempfile::TempDir;

    use crate::console::Console;
    use crate::Result;

    use super::copy_tree;

    /// Test for regression of <https://github.com/sourcefrog/cargo-mutants/issues/450>
    #[test]
    fn copy_tree_with_parent_ignoring_star() -> Result<()> {
        let tmp_dir = TempDir::new().unwrap();
        let tmp = tmp_dir.path();
        write(tmp.join(".gitignore"), "*\n")?;

        let a = Utf8PathBuf::try_from(tmp.join("a")).unwrap();
        create_dir(&a)?;
        write(a.join("Cargo.toml"), "[package]\nname = a")?;
        let src = a.join("src");
        create_dir(&src)?;
        write(src.join("main.rs"), "fn main() {}")?;

        let dest_tmpdir = copy_tree(&a, "a", true, &Console::new())?;
        let dest = dest_tmpdir.path();
        assert!(dest.join("Cargo.toml").is_file());
        assert!(dest.join("src").is_dir());
        assert!(dest.join("src/main.rs").is_file());

        Ok(())
    }
}

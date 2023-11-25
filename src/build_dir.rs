// Copyright 2021-2023 Martin Pool

//! A temporary directory containing mutated source to run cargo builds and tests.

use std::convert::TryInto;
use std::fmt;
use std::fs::FileType;

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use ignore::WalkBuilder;
use tempfile::TempDir;
use tracing::{info, warn};

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
    strategy: TempDirStrategy,
    gitignore: bool,
}

enum TempDirStrategy {
    Collect(TempDir),
    Leak,
}

impl BuildDir {
    /// Make a new build dir, copying from a source directory.
    ///
    /// [SOURCE_EXCLUDE] is excluded.
    pub fn new(source: &Utf8Path, options: &Options, console: &Console) -> Result<BuildDir> {
        let name_base = format!("cargo-mutants-{}-", source.file_name().unwrap_or("unnamed"));
        let source_abs = source
            .canonicalize_utf8()
            .expect("canonicalize source path");
        let temp_dir = copy_tree(source, &name_base, options.gitignore, console)?;
        let path: Utf8PathBuf = temp_dir.path().to_owned().try_into().unwrap();
        fix_manifest(&path.join("Cargo.toml"), &source_abs)?;
        fix_cargo_config(&path, &source_abs)?;
        let strategy = if options.leak_dirs {
            let _ = temp_dir.into_path();
            info!(?path, "Build directory will be leaked for inspection");
            TempDirStrategy::Leak
        } else {
            TempDirStrategy::Collect(temp_dir)
        };
        let build_dir = BuildDir {
            strategy,
            name_base,
            path,
            gitignore: options.gitignore,
        };
        Ok(build_dir)
    }

    pub fn path(&self) -> &Utf8Path {
        self.path.as_path()
    }

    /// Make a copy of this build dir, including its target directory.
    pub fn copy(&self, console: &Console) -> Result<BuildDir> {
        let temp_dir = copy_tree(&self.path, &self.name_base, self.gitignore, console)?;
        Ok(BuildDir {
            path: temp_dir.path().to_owned().try_into().unwrap(),
            strategy: TempDirStrategy::Collect(temp_dir),
            name_base: self.name_base.clone(),
            gitignore: self.gitignore,
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

/// Copy a source tree, with some exclusions, to a new temporary directory.
///
/// If `git` is true, ignore files that are excluded by all the various `.gitignore`
/// files.
///
/// Regardless, anything matching [SOURCE_EXCLUDE] is excluded.
fn copy_tree(
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
            console.copy_progress(bytes_copied);
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
    let link_target = std::fs::read_link(src_path)
        .with_context(|| format!("read link {:?}", src_path.to_slash_lossy()))?;
    if ft.is_symlink_dir() {
        std::os::windows::fs::symlink_dir(link_target, dest_path)
            .with_context(|| format!("create symlink {dest_path:?}"))?;
    } else if ft.is_symlink_file() {
        std::os::windows::fs::symlink_file(link_target, dest_path)
            .with_context(|| format!("create symlink {dest_path:?}"))?;
    } else {
        bail!("Unknown symlink type: {:?}", ft);
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use regex::Regex;

    use super::*;

    #[test]
    fn build_dir_debug_form() {
        let options = Options::default();
        let workspace = Workspace::open("testdata/factorial").unwrap();
        let build_dir = BuildDir::new(&workspace.dir, &options, &Console::new()).unwrap();
        let debug_form = format!("{build_dir:?}");
        assert!(
            Regex::new(r#"^BuildDir \{ path: "[^"]*[/\\]cargo-mutants-factorial[^"]*" \}$"#)
                .unwrap()
                .is_match(&debug_form),
            "debug form is {debug_form:?}",
        );
    }
}

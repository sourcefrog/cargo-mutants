// Copyright 2021-2023 Martin Pool

//! A temporary directory containing mutated source to run cargo builds and tests.

use std::convert::TryInto;
use std::fmt;

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use tempfile::TempDir;
use tracing::{debug, error, info, trace};

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
        let name_base = format!("cargo-mutants-{}-", source.file_name().unwrap_or(""));
        let source_abs = source
            .canonicalize_utf8()
            .expect("canonicalize source path");
        // TODO: Only exclude `target` in directories containing Cargo.toml?
        let temp_dir = copy_tree(source, &name_base, SOURCE_EXCLUDE, console)?;
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
        };
        Ok(build_dir)
    }

    pub fn path(&self) -> &Utf8Path {
        self.path.as_path()
    }

    /// Make a copy of this build dir, including its target directory.
    pub fn copy(&self, console: &Console) -> Result<BuildDir> {
        let temp_dir = copy_tree(&self.path, &self.name_base, &[], console)?;

        Ok(BuildDir {
            path: temp_dir.path().to_owned().try_into().unwrap(),
            strategy: TempDirStrategy::Collect(temp_dir),
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
        .prefix(name_base)
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

#[cfg(test)]
mod test {
    use regex::Regex;

    use super::*;

    #[test]
    fn build_dir_debug_form() {
        let options = Options::default();
        let workspace = Workspace::open(Utf8Path::new("testdata/tree/factorial")).unwrap();
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

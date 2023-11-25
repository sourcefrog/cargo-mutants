// Copyright 2021-2023 Martin Pool

//! A temporary directory containing mutated source to run cargo builds and tests.

use std::convert::TryInto;
use std::fmt;

use camino::{Utf8Path, Utf8PathBuf};
use tempfile::TempDir;
use tracing::info;

use crate::copy_tree::copy_tree;
use crate::manifest::fix_cargo_config;
use crate::*;

/// A temporary directory initialized with a copy of the source, where mutations can be tested.
pub struct BuildDir {
    /// The path of the root of the temporary directory.
    path: Utf8PathBuf,
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
        let build_dir = BuildDir { strategy, path };
        Ok(build_dir)
    }

    pub fn path(&self) -> &Utf8Path {
        self.path.as_path()
    }
}

impl fmt::Debug for BuildDir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BuildDir")
            .field("path", &self.path)
            .finish()
    }
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

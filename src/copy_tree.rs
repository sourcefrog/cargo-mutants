// Copyright 2023 - 2024 Martin Pool

//! Copy a source tree, with some exclusions, to a new temporary directory.

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use ignore::WalkBuilder;
use path_slash::PathExt;
use tempfile::TempDir;
use tracing::{debug, warn};

use crate::options::Options;
use crate::{check_interrupted, Console, Result};

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix::copy_symlink;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows::copy_symlink;

static VCS_DIRS: &[&str] = &[".git", ".hg", ".bzr", ".svn", "_darcs", ".pijul"];

/// Copy a source tree, with some exclusions, to a new temporary directory.
///
/// Regardless, anything matching [SOURCE_EXCLUDE] is excluded.
pub fn copy_tree(
    from_path: &Utf8Path,
    name_base: &str,
    options: &Options,
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
    let copy_vcs = options.copy_vcs; // for lifetime
    walk_builder
        .git_ignore(options.gitignore)
        .git_exclude(options.gitignore)
        .git_global(options.gitignore)
        .hidden(false) // copy hidden files
        .ignore(false) // don't use .ignore
        .require_git(true) // stop at git root; only read gitignore files inside git trees
        .filter_entry(move |entry| {
            let name = entry.file_name().to_string_lossy();
            name != "mutants.out"
                && name != "mutants.out.old"
                && (copy_vcs || !VCS_DIRS.contains(&name.as_ref()))
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
    // TODO: Maybe run these with $HOME set to a temp dir so that global git config has no effect?

    use std::fs::{create_dir, write};

    use camino::Utf8PathBuf;
    use tempfile::TempDir;

    use crate::console::Console;
    use crate::options::Options;
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

        let options = Options::from_arg_strs(["--gitignore=true"]);
        let dest_tmpdir = copy_tree(&a, "a", &options, &Console::new())?;
        let dest = dest_tmpdir.path();
        assert!(dest.join("Cargo.toml").is_file());
        assert!(dest.join("src").is_dir());
        assert!(dest.join("src/main.rs").is_file());

        Ok(())
    }

    /// With `gitignore` set to `true`, but no `.git`, don't exclude anything.
    #[test]
    fn copy_with_gitignore_but_without_git_dir() -> Result<()> {
        let tmp_dir = TempDir::new().unwrap();
        let tmp = Utf8PathBuf::try_from(tmp_dir.path().to_owned()).unwrap();
        write(tmp.join(".gitignore"), "foo\n")?;

        write(tmp.join("Cargo.toml"), "[package]\nname = a")?;
        let src = tmp.join("src");
        create_dir(&src)?;
        write(src.join("main.rs"), "fn main() {}")?;
        write(tmp.join("foo"), "bar")?;

        let options = Options::from_arg_strs(["--gitignore=true"]);
        let dest_tmpdir = copy_tree(&tmp, "a", &options, &Console::new())?;
        let dest = dest_tmpdir.path();
        assert!(
            dest.join("foo").is_file(),
            "foo should be copied because gitignore is not used without .git"
        );

        Ok(())
    }

    /// With `gitignore` set to `true`, in a tree with `.git`, `.gitignore` is respected.
    #[test]
    fn copy_with_gitignore_and_git_dir() -> Result<()> {
        let tmp_dir = TempDir::new().unwrap();
        let tmp = Utf8PathBuf::try_from(tmp_dir.path().to_owned()).unwrap();
        write(tmp.join(".gitignore"), "foo\n")?;
        create_dir(tmp.join(".git"))?;

        write(tmp.join("Cargo.toml"), "[package]\nname = a")?;
        let src = tmp.join("src");
        create_dir(&src)?;
        write(src.join("main.rs"), "fn main() {}")?;
        write(tmp.join("foo"), "bar")?;

        let options = Options::from_arg_strs(["mutants", "--gitignore=true"]);
        let dest_tmpdir = copy_tree(&tmp, "a", &options, &Console::new())?;
        let dest = dest_tmpdir.path();
        assert!(
            !dest.join("foo").is_file(),
            "foo should have been excluded by gitignore"
        );

        Ok(())
    }

    /// With `gitignore` set to `false`, patterns in that file have no effect.
    #[test]
    fn copy_without_gitignore() -> Result<()> {
        let tmp_dir = TempDir::new().unwrap();
        let tmp = Utf8PathBuf::try_from(tmp_dir.path().to_owned()).unwrap();
        write(tmp.join(".gitignore"), "foo\n")?;
        create_dir(tmp.join(".git"))?;

        write(tmp.join("Cargo.toml"), "[package]\nname = a")?;
        let src = tmp.join("src");
        create_dir(&src)?;
        write(src.join("main.rs"), "fn main() {}")?;
        write(tmp.join("foo"), "bar")?;

        let options = Options::from_arg_strs(["mutants", "--gitignore=false"]);
        let dest_tmpdir = copy_tree(&tmp, "a", &options, &Console::new())?;
        let dest = dest_tmpdir.path();
        // gitignore didn't exclude `foo`
        assert!(dest.join("foo").is_file());

        Ok(())
    }

    #[test]
    fn dont_copy_git_dir_or_mutants_out_by_default() -> Result<()> {
        let tmp_dir = TempDir::new().unwrap();
        let tmp = Utf8PathBuf::try_from(tmp_dir.path().to_owned()).unwrap();
        create_dir(tmp.join(".git"))?;
        write(tmp.join(".git/foo"), "bar")?;
        create_dir(tmp.join("mutants.out"))?;
        write(tmp.join("mutants.out/foo"), "bar")?;

        write(tmp.join("Cargo.toml"), "[package]\nname = a")?;
        let src = tmp.join("src");
        create_dir(&src)?;
        write(src.join("main.rs"), "fn main() {}")?;

        let options = Options::from_arg_strs(["mutants"]);
        let dest_tmpdir = copy_tree(&tmp, "a", &options, &Console::new())?;
        let dest = dest_tmpdir.path();
        assert!(!dest.join(".git").is_dir(), ".git should not be copied");
        assert!(
            !dest.join(".git/foo").is_file(),
            ".git/foo should not be copied"
        );
        assert!(
            !dest.join("mutants.out").exists(),
            "mutants.out should not be copied"
        );
        assert!(
            dest.join("Cargo.toml").is_file(),
            "Cargo.toml should be copied"
        );

        Ok(())
    }

    #[test]
    fn copy_git_dir_when_requested() -> Result<()> {
        let tmp_dir = TempDir::new().unwrap();
        let tmp = Utf8PathBuf::try_from(tmp_dir.path().to_owned()).unwrap();
        create_dir(tmp.join(".git"))?;
        write(tmp.join(".git/foo"), "bar")?;
        create_dir(tmp.join("mutants.out"))?;
        write(tmp.join("mutants.out/foo"), "bar")?;

        write(tmp.join("Cargo.toml"), "[package]\nname = a")?;
        let src = tmp.join("src");
        create_dir(&src)?;
        write(src.join("main.rs"), "fn main() {}")?;

        let options = Options::from_arg_strs(["mutants", "--copy-vcs=true"]);
        let dest_tmpdir = copy_tree(&tmp, "a", &options, &Console::new())?;
        let dest = dest_tmpdir.path();
        assert!(dest.join(".git").is_dir(), ".git should be copied");
        assert!(dest.join(".git/foo").is_file(), ".git/foo should be copied");
        assert!(
            !dest.join("mutants.out").exists(),
            "mutants.out should not be copied"
        );
        assert!(
            dest.join("Cargo.toml").is_file(),
            "Cargo.toml should be copied"
        );

        Ok(())
    }
}

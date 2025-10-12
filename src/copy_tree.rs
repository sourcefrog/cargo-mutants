// Copyright 2023 - 2025 Martin Pool

//! Copy a source tree, with some exclusions, to a new temporary directory.

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use ignore::WalkBuilder;
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

static VCS_DIRS: &[&str] = &[".git", ".hg", ".bzr", ".svn", "_darcs", ".jj", ".pijul"];

/// Copy a file, attempting to use reflink if supported.
/// Returns the number of bytes copied.
fn copy_file(src: &Path, dest: &Path, reflink_supported: &AtomicBool) -> Result<u64> {
    // Try reflink first if we haven't determined it's not supported
    if reflink_supported.load(Ordering::Relaxed) {
        match reflink::reflink(src, dest) {
            Ok(()) => {
                // Reflink succeeded, get file size for progress tracking
                let metadata = fs::metadata(dest)
                    .with_context(|| format!("Failed to get metadata for {}", dest.display()))?;
                return Ok(metadata.len());
            }
            Err(e) => {
                // On Windows, reflink can fail without returning ErrorKind::Unsupported,
                // so we give up on reflinks after any error to avoid repeated failures.
                reflink_supported.store(false, Ordering::Relaxed);
                debug!("Reflink failed: {}, falling back to regular copy", e);
            }
        }
    }

    // Fall back to regular copy
    fs::copy(src, dest)
        .with_context(|| format!("Failed to copy {} to {}", src.display(), dest.display()))
}

/// Copy a source tree, with some exclusions, to a new temporary directory.
pub fn copy_tree(
    from_path: &Utf8Path,
    name_base: &str,
    options: &Options,
    console: &Console,
) -> Result<TempDir> {
    let mut total_bytes = 0;
    let mut total_files = 0;
    let reflink_supported = AtomicBool::new(true);
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
    let from_path_owned = from_path.to_owned(); // for lifetime in closure
    let copy_target = options.copy_target;
    walk_builder
        .git_ignore(options.gitignore)
        .git_exclude(options.gitignore)
        .git_global(options.gitignore)
        .hidden(false) // copy hidden files
        .ignore(false) // don't use .ignore
        .require_git(true) // stop at git root; only read gitignore files inside git trees
        .filter_entry(move |entry| {
            let name = entry.file_name().to_string_lossy();
            let is_top_level_target = name == "target"
                && entry
                    .path()
                    .parent()
                    .is_some_and(|p| p == from_path_owned.as_path());
            name != "mutants.out"
                && name != "mutants.out.old"
                && (copy_target || !is_top_level_target)
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
        let ft = entry.file_type().with_context(|| {
            format!(
                "Expected file to have a file type: {}",
                entry.path().display()
            )
        })?;
        if ft.is_file() {
            let bytes_copied =
                copy_file(entry.path(), dest_path.as_std_path(), &reflink_supported)?;
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
    let reflink_used = reflink_supported.load(Ordering::Relaxed);
    debug!(?total_bytes, ?total_files, ?reflink_used, temp_dir = ?temp_dir.path(), "Copied source tree");
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

    /// With `gitignore` set to `false` (the default), patterns in that file have no effect.
    #[test]
    fn copy_without_gitignore_by_default() -> Result<()> {
        let tmp_dir = TempDir::new().unwrap();
        let tmp = Utf8PathBuf::try_from(tmp_dir.path().to_owned()).unwrap();
        write(tmp.join(".gitignore"), "foo\n")?;
        create_dir(tmp.join(".git"))?;

        write(tmp.join("Cargo.toml"), "[package]\nname = a")?;
        let src = tmp.join("src");
        create_dir(&src)?;
        write(src.join("main.rs"), "fn main() {}")?;
        write(tmp.join("foo"), "bar")?;

        let options = Options::from_arg_strs(["mutants"]);
        let dest_tmpdir = copy_tree(&tmp, "a", &options, &Console::new())?;
        let dest = dest_tmpdir.path();
        // gitignore didn't exclude `foo` because gitignore is false by default
        assert!(dest.join("foo").is_file());

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

    #[test]
    fn copy_with_gitignore_true_in_config_and_git_dir_excludes_ignored_files() -> Result<()> {
        let tmp_dir = TempDir::new().unwrap();
        let tmp = Utf8PathBuf::try_from(tmp_dir.path().to_owned()).unwrap();
        write(tmp.join(".gitignore"), "foo\n")?;
        create_dir(tmp.join(".git"))?;

        write(tmp.join("Cargo.toml"), "[package]\nname = a")?;
        let src = tmp.join("src");
        create_dir(&src)?;
        write(src.join("main.rs"), "fn main() {}")?;
        write(tmp.join("foo"), "bar")?;

        let options = Options::from_arg_strs_and_config(["mutants"], "gitignore=true");
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
        assert!(!dest.join("target").exists(), "target should not be copied");
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

    #[test]
    fn dont_copy_target_dir_by_default_when_copy_target_false() -> Result<()> {
        let tmp_dir = TempDir::new().unwrap();
        let tmp = Utf8PathBuf::try_from(tmp_dir.path().to_owned()).unwrap();
        create_dir(tmp.join("target"))?;
        write(tmp.join("target/foo"), "bar")?;

        write(tmp.join("Cargo.toml"), "[package]\nname = a")?;
        let src = tmp.join("src");
        create_dir(&src)?;
        write(src.join("main.rs"), "fn main() {}")?;

        let options = Options::from_arg_strs(["mutants"]);
        let dest_tmpdir = copy_tree(&tmp, "a", &options, &Console::new())?;
        let dest = dest_tmpdir.path();
        assert!(
            !dest.join("target").exists(),
            "target should not be copied by default"
        );
        assert!(
            dest.join("Cargo.toml").is_file(),
            "Cargo.toml should be copied"
        );

        Ok(())
    }

    #[test]
    fn copy_target_dir_when_requested() -> Result<()> {
        let tmp_dir = TempDir::new().unwrap();
        let tmp = Utf8PathBuf::try_from(tmp_dir.path().to_owned()).unwrap();
        create_dir(tmp.join("target"))?;
        write(tmp.join("target/foo"), "bar")?;

        write(tmp.join("Cargo.toml"), "[package]\nname = a")?;
        let src = tmp.join("src");
        create_dir(&src)?;
        write(src.join("main.rs"), "fn main() {}")?;

        let options = Options::from_arg_strs(["mutants", "--copy-target=true"]);
        let dest_tmpdir = copy_tree(&tmp, "a", &options, &Console::new())?;
        let dest = dest_tmpdir.path();
        assert!(
            dest.join("target").exists(),
            "target should be copied when --copy-target=true"
        );
        assert!(
            dest.join("target/foo").is_file(),
            "target/foo should be copied when --copy-target=true"
        );
        assert!(
            dest.join("Cargo.toml").is_file(),
            "Cargo.toml should be copied"
        );

        Ok(())
    }

    #[test]
    fn copy_non_top_level_target_files() -> Result<()> {
        let tmp_dir = TempDir::new().unwrap();
        let tmp = Utf8PathBuf::try_from(tmp_dir.path().to_owned()).unwrap();

        // Create top-level target directory (should be excluded)
        create_dir(tmp.join("target"))?;
        write(tmp.join("target/build_artifact"), "should not be copied")?;

        // Create non-top-level target file and directory (should be copied)
        let testdata = tmp.join("testdata");
        create_dir(&testdata)?;
        write(testdata.join("target"), "should be copied")?;

        let subdir = tmp.join("subdir");
        create_dir(&subdir)?;
        create_dir(subdir.join("target"))?;
        write(subdir.join("target/file"), "should be copied")?;

        write(tmp.join("Cargo.toml"), "[package]\nname = a")?;
        let src = tmp.join("src");
        create_dir(&src)?;
        write(src.join("main.rs"), "fn main() {}")?;

        let options = Options::from_arg_strs(["mutants"]);
        let dest_tmpdir = copy_tree(&tmp, "a", &options, &Console::new())?;
        let dest = dest_tmpdir.path();

        // Top-level target should be excluded
        assert!(
            !dest.join("target").exists(),
            "top-level target directory should not be copied"
        );

        // Non-top-level target files/dirs should be included
        assert!(
            dest.join("testdata/target").is_file(),
            "testdata/target file should be copied"
        );
        assert!(
            dest.join("subdir/target").is_dir(),
            "subdir/target directory should be copied"
        );
        assert!(
            dest.join("subdir/target/file").is_file(),
            "subdir/target/file should be copied"
        );

        Ok(())
    }
}

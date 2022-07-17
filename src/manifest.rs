// Copyright 2022 Martin Pool.

//! Manipulate Cargo.toml manifest files.

use std::fs;

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};

use crate::Result;

/// Rewrite the scratch copy of a manifest to have absolute paths.
///
/// `manifest_source_dir` is the directory originally containing the manifest, from
/// which the absolute paths are calculated.
pub fn fix_manifest(
    manifest_scratch_path: &Utf8Path,
    manifest_source_dir: &Utf8Path,
) -> Result<()> {
    // eprintln!(
    //     "fixing manifest {} relative to {}",
    //     manifest_scratch_path, &manifest_source_dir
    // );
    let toml_str = fs::read_to_string(manifest_scratch_path).context("read manifest")?;
    if let Some(changed_toml) = fix_manifest_toml_str(&toml_str, manifest_source_dir)? {
        fs::write(manifest_scratch_path, changed_toml.as_bytes()).context("write manifest")?;
    }
    Ok(())
}

/// Fix any relative paths within a Cargo.toml manifest.
///
/// Returns the new manifest, or None if no changes were made.
fn fix_manifest_toml_str(
    manifest_toml_str: &str,
    manifest_source_dir: &Utf8Path,
) -> Result<Option<String>> {
    // TODO: Also look at `patch` and `replace` sections.
    let mut value: toml::Value = manifest_toml_str.parse().context("parse manifest")?;
    let orig_value = value.clone();
    // dbg!(&value);
    if let Some(top_table) = value.as_table_mut() {
        if let Some(dependencies) = top_table.get_mut("dependencies") {
            if let Some(dependencies_table) = dependencies.as_table_mut() {
                for (_dependency_name, value) in dependencies_table.iter_mut() {
                    if let Some(dependency_table) = value.as_table_mut() {
                        if let Some(path_value) = dependency_table.get_mut("path") {
                            // eprintln!(
                            //     "found dependency {dependency_name} with path {}",
                            //     path_value.as_str().unwrap_or("???")
                            // );
                            if let Some(path_str) = path_value.as_str() {
                                if let Some(new_path) = fix_path(path_str, manifest_source_dir) {
                                    *path_value = toml::Value::String(new_path.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    if value == orig_value {
        Ok(None)
    } else {
        Ok(Some(toml::to_string_pretty(&value)?))
    }
}

/// Fix one path, from inside a scratch tree, to be absolute as interpreted relative to the source tree.
fn fix_path(path_str: &str, manifest_source_dir: &Utf8Path) -> Option<Utf8PathBuf> {
    let path = Utf8Path::new(path_str);
    if path.is_absolute() {
        None
    } else {
        let mut new_path = manifest_source_dir.to_owned();
        new_path.push(path);
        Some(new_path)
    }
}

#[cfg(test)]
mod test {
    use camino::Utf8Path;

    #[test]
    fn fix_path_absolute_unchanged() {
        let dependency_abspath = Utf8Path::new("testdata/tree/dependency")
            .canonicalize_utf8()
            .unwrap();
        assert_eq!(
            super::fix_path(
                dependency_abspath.as_str(),
                &Utf8Path::new("/home/user/src/foo")
            ),
            None
        );
    }

    #[test]
    fn fix_path_relative() {
        assert_eq!(
            super::fix_path(
                "../dependency",
                &Utf8Path::new("testdata/tree/relative_dependency")
            ),
            Some(Utf8Path::new("testdata/tree/relative_dependency/../dependency").to_owned())
        );
    }
}

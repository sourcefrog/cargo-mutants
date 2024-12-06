// Copyright 2022-2024 Martin Pool.

//! Manipulate Cargo manifest and config files.
//!
//! In particular, when the tree is copied we have to fix up relative paths, so
//! that they still work from the new location of the scratch directory.

use std::fs::{read_to_string, write};

use anyhow::Context;
use camino::Utf8Path;
use tracing::debug;

use crate::path::ascent;
use crate::Result;

/// Rewrite the scratch copy of a manifest to have absolute paths.
///
/// `manifest_source_dir` is the directory originally containing the manifest, from
/// which the absolute paths are calculated.
#[allow(clippy::module_name_repetitions)]
pub fn fix_manifest(manifest_scratch_path: &Utf8Path, source_dir: &Utf8Path) -> Result<()> {
    let toml_str = read_to_string(manifest_scratch_path).with_context(|| {
        format!("failed to read manifest from build directory: {manifest_scratch_path}")
    })?;
    if let Some(changed_toml) = fix_manifest_toml(&toml_str, source_dir)? {
        let toml_str =
            toml::to_string_pretty(&changed_toml).context("serialize changed manifest")?;
        write(manifest_scratch_path, toml_str.as_bytes()).with_context(|| {
            format!("Failed to write fixed manifest to {manifest_scratch_path}")
        })?;
    }
    Ok(())
}

/// Fix any relative paths within a Cargo.toml manifest.
///
/// Returns the new manifest, or None if no changes were made.
fn fix_manifest_toml(
    manifest_toml: &str,
    manifest_source_dir: &Utf8Path,
) -> Result<Option<toml::Value>> {
    let mut value: toml::Value = manifest_toml.parse().context("parse manifest")?;
    let orig_value = value.clone();
    if let Some(top_table) = value.as_table_mut() {
        if let Some(dependencies) = top_table.get_mut("dependencies") {
            fix_dependency_table(dependencies, manifest_source_dir);
        }
        if let Some(replace) = top_table.get_mut("replace") {
            // The replace section is a table from package name/version to a
            // table which might include a `path` key. (The keys are not exactly
            // package names but it doesn't matter.)
            // <https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html#the-replace-section>
            fix_dependency_table(replace, manifest_source_dir);
        }
        if let Some(patch_table) = top_table.get_mut("patch").and_then(|p| p.as_table_mut()) {
            // The keys of the patch table are registry names or source URLs;
            // the values are like dependency tables.
            // <https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html#the-patch-section>
            for (_name, dependencies) in patch_table {
                fix_dependency_table(dependencies, manifest_source_dir);
            }
        }
    }
    if value == orig_value {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

/// Fix up paths in a manifest "dependency table".
///
/// This is a pattern that can occur at various places in the manifest. It's a
/// map from a string (such as a package name) to a table which may contain a "path" field.
///
/// For example:
///
/// ```yaml
/// mutants = { version = "1.0", path = "../mutants" }
/// ```
///
/// The table is mutated if necessary.
///
/// `dependencies` is a TOML Value that should normally be a table;
/// other values are left unchanged.
///
/// Entries that have no `path` are left unchanged too.
fn fix_dependency_table(dependencies: &mut toml::Value, manifest_source_dir: &Utf8Path) {
    if let Some(dependencies_table) = dependencies.as_table_mut() {
        for (_, value) in dependencies_table.iter_mut() {
            if let Some(dependency_table) = value.as_table_mut() {
                if let Some(path_value) = dependency_table.get_mut("path") {
                    if let Some(path_str) = path_value.as_str() {
                        if let Some(new_path) = fix_path(path_str, manifest_source_dir) {
                            *path_value = toml::Value::String(new_path);
                        }
                    }
                }
            }
        }
    }
}

/// Rewrite relative paths within `.cargo/config.toml` to be absolute paths.
pub fn fix_cargo_config(build_path: &Utf8Path, source_path: &Utf8Path) -> Result<()> {
    let config_path = build_path.join(".cargo/config.toml");
    if config_path.exists() {
        let toml_str = read_to_string(&config_path).context("read .cargo/config.toml")?;
        if let Some(changed_toml) = fix_cargo_config_toml(&toml_str, source_path)? {
            write(build_path.join(&config_path), changed_toml.as_bytes())
                .context("write .cargo/config.toml")?;
        }
    }
    Ok(())
}

/// Replace any relative paths in a config file with absolute paths.
///
/// Returns None if no changes are needed.
///
/// See <https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html?search=#paths-overrides>.
fn fix_cargo_config_toml(config_toml: &str, source_dir: &Utf8Path) -> Result<Option<String>> {
    let mut value: toml::Value = config_toml.parse().context("parse config.toml")?;
    let mut changed = false;
    if let Some(paths) = value.get_mut("paths").and_then(|p| p.as_array_mut()) {
        for path_value in paths {
            if let Some(path_str) = path_value.as_str() {
                if let Some(new_path) = fix_path(path_str, source_dir) {
                    *path_value = toml::Value::String(new_path);
                    changed = true;
                }
            }
        }
    }
    if changed {
        Ok(Some(toml::to_string_pretty(&value)?))
    } else {
        Ok(None)
    }
}

/// Fix one path, from inside a scratch tree, to be absolute as interpreted
/// relative to the source tree.
///
/// Paths pointing into a subdirectory of the source tree are left unchanged.
///
/// Returns None if the path does not need to be changed.
fn fix_path(path_str: &str, source_dir: &Utf8Path) -> Option<String> {
    let path = Utf8Path::new(path_str);
    if path.is_absolute() || ascent(path) == 0 {
        None
    } else {
        let mut new_path = source_dir.to_owned();
        new_path.push(path);
        let new_path_str = new_path.to_string();
        debug!("fix path {path_str} -> {new_path_str}");
        Some(new_path_str)
    }
}

#[cfg(test)]
mod test {
    use camino::{Utf8Path, Utf8PathBuf};
    use indoc::indoc;
    use pretty_assertions::assert_eq;
    use toml::Table;

    use super::{fix_cargo_config_toml, fix_manifest_toml};

    #[test]
    fn fix_path_absolute_unchanged() {
        let dependency_abspath = Utf8Path::new("testdata/dependency")
            .canonicalize_utf8()
            .unwrap();
        assert_eq!(
            super::fix_path(
                dependency_abspath.as_str(),
                Utf8Path::new("/home/user/src/foo")
            ),
            None
        );
    }

    #[test]
    fn fix_path_relative() {
        let fixed_path: Utf8PathBuf = super::fix_path(
            "../dependency",
            Utf8Path::new("testdata/relative_dependency"),
        )
        .expect("path was adjusted")
        .into();
        assert_eq!(
            &fixed_path,
            Utf8Path::new("testdata/relative_dependency/../dependency"),
        );
    }

    #[test]
    fn fix_relative_path_in_manifest() {
        let manifest_toml = indoc! { r#"
            # A comment, which will be dropped.
            author = "A Smithee"
            [dependencies]
            wibble = { path = "../wibble" } # Use the relative path to the dependency.
        "# };
        let orig_path = Utf8Path::new("/home/user/src/foo");
        let fixed = fix_manifest_toml(manifest_toml, orig_path)
            .unwrap()
            .expect("toml was modified");
        println!("{fixed:#?}");
        assert_eq!(fixed["author"].as_str().unwrap(), "A Smithee");
        assert_eq!(
            fixed["dependencies"]["wibble"]["path"].as_str().unwrap(),
            Utf8Path::new("/home/user/src/foo/../wibble")
        );
    }

    #[test]
    fn fix_replace_section() {
        let manifest_toml = indoc! { r#"
            [dependencies]
            wibble = "1.2.3"
            [replace]
            "wibble:1.2.3" = { path = "../wibble" } # Use the relative path to the dependency.
        "# };
        let orig_path = Utf8Path::new("/home/user/src/foo");
        let fixed = fix_manifest_toml(manifest_toml, orig_path)
            .unwrap()
            .expect("toml was modified");
        println!("fixed toml:\n{}", toml::to_string_pretty(&fixed).unwrap());
        assert_eq!(fixed["dependencies"]["wibble"].as_str().unwrap(), "1.2.3");
        assert_eq!(
            fixed["replace"]["wibble:1.2.3"]["path"].as_str().unwrap(),
            orig_path.join("../wibble")
        );
    }

    #[test]
    fn absolute_path_in_manifest_is_unchanged() {
        #[cfg(unix)]
        let manifest_toml = indoc! { r#"
            [dependencies]
            wibble = { path = "/home/asmithee/src/wibble" }
        "# };
        #[cfg(windows)]
        let manifest_toml = indoc! { r#"
            [dependencies]
            wibble = { path = "c:/home/asmithee/src/wibble" }
        "# };

        let orig_path = Utf8Path::new("/home/user/src/foo");
        let fixed_toml = fix_manifest_toml(manifest_toml, orig_path).unwrap();
        assert_eq!(
            fixed_toml, None,
            "manifest containing only an absolute path should not be modified"
        );
    }

    #[test]
    fn subdir_path_in_manifest_is_unchanged() {
        let manifest_toml = indoc! { r#"
            [dependencies]
            wibble = { path = "wibble" }
        "# };

        let orig_path = Utf8Path::new("/home/user/src/foo");
        let fixed_toml = fix_manifest_toml(manifest_toml, orig_path).unwrap();
        assert_eq!(
            fixed_toml, None,
            "manifest with a relative path to a subdirectory should not be modified",
        );
    }

    #[test]
    fn fix_patch_section() {
        let manifest_toml = indoc! { r#"
            [dependencies]
            wibble = "1.2.3"
            [patch.crates-io]
            wibble = { path = "../wibble" } # Use the relative path to the dependency.
        "# };
        let orig_path = Utf8Path::new("/home/user/src/foo");
        let fixed = fix_manifest_toml(manifest_toml, orig_path)
            .unwrap()
            .expect("toml was modified");
        println!("{fixed:#?}");
        assert_eq!(fixed["dependencies"]["wibble"].as_str(), Some("1.2.3"));
        assert_eq!(
            fixed["patch"]["crates-io"]["wibble"]["path"]
                .as_str()
                .unwrap(),
            orig_path.join("../wibble")
        );
    }

    #[test]
    fn cargo_config_toml_paths_outside_tree_are_made_absolute() {
        // To avoid test flakiness due to TOML stylistic changes, we compare the
        // TOML values.
        //
        // And, to avoid headaches about forward and backslashes on Windows,
        // compare path objects.
        let cargo_config_toml = indoc! { r#"
            paths = [
                "sub_dependency",
                "../sibling_dependency",
                "../../parent_dependency",
                "/Users/jane/src/absolute_dependency",
                "/src/other",
            ]"# };
        let source_dir = Utf8Path::new("/Users/jane/src/foo");
        let fixed_toml = fix_cargo_config_toml(cargo_config_toml, source_dir)
            .unwrap()
            .expect("toml was modified");
        println!("fixed toml:\n{fixed_toml}");
        // TODO: Maybe fix_cargo_config_toml should return the Value.
        let fixed_table: Table = fixed_toml.parse::<Table>().unwrap();
        let fixed_paths = fixed_table["paths"]
            .as_array()
            .unwrap()
            .iter()
            .map(|val| val.as_str().unwrap().into())
            .collect::<Vec<&Utf8Path>>();
        assert_eq!(
            fixed_paths,
            [
                Utf8Path::new("sub_dependency"),
                &source_dir.join("../sibling_dependency"),
                &source_dir.join("../../parent_dependency"),
                &source_dir.parent().unwrap().join("absolute_dependency"),
                Utf8Path::new("/src/other"),
            ]
        );
    }
}

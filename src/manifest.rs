// Copyright 2022 Martin Pool.

//! Manipulate Cargo manifest and config files.
//!
//! In particular, when the tree is copied we have to fix up relative paths, so
//! that they still work from the new location of the scratch directory.

use std::fs;

use anyhow::Context;
use camino::Utf8Path;

use crate::Result;

/// Rewrite the scratch copy of a manifest to have absolute paths.
///
/// `manifest_source_dir` is the directory originally containing the manifest, from
/// which the absolute paths are calculated.
pub fn fix_manifest(manifest_scratch_path: &Utf8Path, source_dir: &Utf8Path) -> Result<()> {
    let toml_str = fs::read_to_string(manifest_scratch_path).context("read manifest")?;
    if let Some(changed_toml) = fix_manifest_toml(&toml_str, source_dir)? {
        fs::write(manifest_scratch_path, changed_toml.as_bytes()).context("write manifest")?;
    }
    Ok(())
}

/// Fix any relative paths within a Cargo.toml manifest.
///
/// Returns the new manifest, or None if no changes were made.
fn fix_manifest_toml(
    manifest_toml: &str,
    manifest_source_dir: &Utf8Path,
) -> Result<Option<String>> {
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
        Ok(Some(toml::to_string_pretty(&value)?))
    }
}

/// Fix up paths in a manifest "dependency table".
///
/// This is a pattern that can occur at various places in the manifest. It's a
/// map from package name to a table which may contain a "path" field.
///
/// The table is mutated if necessary.
///
/// `dependencies_table` is a TOML Value that should normally be a table;
/// other values are ignored.
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
        let toml_str = fs::read_to_string(&config_path).context("read .cargo/config.toml")?;
        if let Some(changed_toml) = fix_cargo_config_toml(&toml_str, source_path)? {
            fs::write(build_path.join(&config_path), changed_toml.as_bytes())
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

/// Fix a dependency path to be absolute, if necessary.
///
/// `path_str` is the path as it occurs in the original manifest or config.
///
/// `package_dir` is the absolute path of the package where this dependency occurs.
/// If `path_str` is relative and must be made absolute, that's done relative to
/// this location.
///
/// Returns None if the path doesn't need to be changed, otherwise the new
/// absolute path.
fn fix_path(path_str: &str, package_dir: &Utf8Path) -> Option<String> {
    debug_assert!(package_dir.is_absolute());
    let path = Utf8Path::new(path_str);
    if path.is_absolute() {
        None
    } else {
        // We don't canonicalize here because that requires that the path exists, and
        // and it's easier to test if we don't require that.
        Some(package_dir.join(path).to_string())
    }
    // TODO: Measure how many directories upward this path ever moves.
}

#[cfg(test)]
mod test {
    use camino::{Utf8Path, Utf8PathBuf};
    use pretty_assertions::assert_eq;

    use super::fix_manifest_toml;

    // #[test]
    // fn fix_path_into_subdir_is_unchanged() {
    //     // This is a relative dependency pointing to a subdirectory of the source tree.
    //     // It's OK to leave it as relative because the whole tree will be copied,
    //     // so the subdirectory can still be found at the same relative position.
    //     let package_dir = Utf8Path::new("testdata/tree/relative_dependency")
    //         .canonicalize_utf8()
    //         .unwrap();
    //     assert_eq!(
    //         super::fix_path("subpackage", &package_dir).unwrap(),
    //         "subpackage",
    //         "TODO"
    //     );
    // }

    #[test]
    fn fix_path_absolute_unchanged() {
        let dependency_abspath = Utf8Path::new("testdata/tree/dependency")
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
        let package_path = Utf8Path::new("testdata/tree/relative_dependency")
            .canonicalize_utf8()
            .unwrap();
        let fixed_path: Utf8PathBuf = super::fix_path("../dependency", &package_path)
            .expect("path was adjusted")
            .into();
        assert_eq!(fixed_path, package_path.join("../dependency"));
    }

    #[test]
    fn fix_relative_path_in_manifest() {
        let manifest_toml = r#"
# A comment, which will be dropped.
author = "A Smithee"
[dependencies]
wibble = { path = "../wibble" } # Use the relative path to the dependency.
"#;
        let orig_path = Utf8Path::new("/home/user/src/foo");
        let fixed_toml = fix_manifest_toml(manifest_toml, orig_path)
            .unwrap()
            .expect("toml was modified");
        // Round-tripping toml produces some insignificant stylistic changes.
        #[cfg(unix)]
        let expected = "author = 'A Smithee'
[dependencies.wibble]
path = '/home/user/src/foo/../wibble'
";
        #[cfg(windows)]
        let expected = "author = 'A Smithee'
[dependencies.wibble]
path = '/home/user/src/foo\\../wibble'
";
        assert_eq!(fixed_toml, expected);
    }

    #[test]
    fn fix_replace_section() {
        let manifest_toml = r#"
[dependencies]
wibble = "1.2.3"
[replace]
"wibble:1.2.3" = { path = "../wibble" } # Use the relative path to the dependency.
"#;
        let orig_path = Utf8Path::new("/home/user/src/foo");
        let fixed_toml = fix_manifest_toml(manifest_toml, orig_path)
            .unwrap()
            .expect("toml was modified");
        // A crude adaption for Windows.
        let fixed_toml = fixed_toml.replace('\\', "/");
        // Round-tripping toml produces some insignificant stylistic changes.
        let expected = r#"[dependencies]
wibble = '1.2.3'
[replace."wibble:1.2.3"]
path = '/home/user/src/foo/../wibble'
"#;
        assert_eq!(fixed_toml, expected);
    }

    #[test]
    fn absolute_path_in_manifest_is_unchanged() {
        #[cfg(unix)]
        let manifest_toml = r#"
[dependencies]
wibble = { path = "/home/asmithee/src/wibble" }
"#;
        #[cfg(windows)]
        let manifest_toml = r#"
[dependencies]
wibble = { path = "c:/home/asmithee/src/wibble" }
"#;

        let orig_path = Utf8Path::new("/home/user/src/foo");
        let fixed_toml = fix_manifest_toml(manifest_toml, orig_path).unwrap();
        assert_eq!(
            fixed_toml, None,
            "manifest containing only an absolute path should not be modified"
        );
    }

    #[test]
    fn fix_patch_section() {
        let manifest_toml = r#"
[dependencies]
wibble = "1.2.3"
[patch.crates-io]
wibble = { path = "../wibble" } # Use the relative path to the dependency.
"#;
        let orig_path = Utf8Path::new("/home/user/src/foo");
        let fixed_toml = fix_manifest_toml(manifest_toml, orig_path)
            .unwrap()
            .expect("toml was modified");
        // A crude adaption for Windows.
        let fixed_toml = fixed_toml.replace('\\', "/");
        // Round-tripping toml produces some insignificant stylistic changes.
        let expected = r#"[dependencies]
wibble = '1.2.3'
[patch.crates-io.wibble]
path = '/home/user/src/foo/../wibble'
"#;
        assert_eq!(fixed_toml, expected);
    }

    #[test]
    fn fix_cargo_config_toml() {
        let cargo_config_toml = r#"
paths = [
    "sub_dependency",
    "../sibling_dependency",
    "../../parent_dependency",
    "/Users/jane/src/absolute_dependency",
    "/src/other",
    ]"#;
        let source_dir = Utf8Path::new("/Users/jane/src/foo");
        let fixed_toml = super::fix_cargo_config_toml(cargo_config_toml, source_dir)
            .unwrap()
            .expect("toml was modified");
        // a crude adaption for windows.
        let fixed_toml = fixed_toml.replace('\\', "/");
        // Round-tripping toml produces some insignificant stylistic changes.
        let expected = r#"paths = [
    '/Users/jane/src/foo/sub_dependency',
    '/Users/jane/src/foo/../sibling_dependency',
    '/Users/jane/src/foo/../../parent_dependency',
    '/Users/jane/src/absolute_dependency',
    '/src/other',
]
"#;
        assert_eq!(fixed_toml, expected);
    }
}

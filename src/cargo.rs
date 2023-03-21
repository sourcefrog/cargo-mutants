// Copyright 2021-2023 Martin Pool

//! Run Cargo as a subprocess, including timeouts and propagating signals.

use std::env;
use std::sync::Arc;

#[allow(unused_imports)]
use anyhow::{anyhow, bail, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use serde_json::Value;
#[allow(unused_imports)]
use tracing::{debug, error, info, span, trace, warn, Level};

use crate::path::TreeRelativePathBuf;
use crate::process::get_command_output;
use crate::source::Package;
use crate::tool::Tool;
use crate::*;

#[derive(Debug)]
pub struct CargoTool {
    // environment is currently constant across all invocations.
    env: Vec<(String, String)>,
}

impl CargoTool {
    pub fn new() -> CargoTool {
        let env = vec![
            ("CARGO_ENCODED_RUSTFLAGS".to_owned(), rustflags()),
            // The tests might use Insta <https://insta.rs>, and we don't want it to write
            // updates to the source tree, and we *certainly* don't want it to write
            // updates and then let the test pass.
            ("INSTA_UPDATE".to_owned(), "no".to_owned()),
        ];
        CargoTool { env }
    }
}

impl Tool for CargoTool {
    fn name(&self) -> &str {
        "cargo"
    }

    fn find_root(&self, path: &Utf8Path) -> Result<Utf8PathBuf> {
        let cargo_toml_path = locate_cargo_toml(path)?;
        let root = cargo_toml_path
            .parent()
            .expect("cargo_toml_path has a parent")
            .to_owned();
        assert!(root.is_dir());
        Ok(root)
    }

    fn root_files(&self, source_root_path: &Utf8Path) -> Result<Vec<Arc<SourceFile>>> {
        let cargo_toml_path = source_root_path.join("Cargo.toml");
        debug!(?cargo_toml_path);
        check_interrupted()?;
        let metadata = cargo_metadata::MetadataCommand::new()
            .manifest_path(&cargo_toml_path)
            .exec()
            .context("run cargo metadata")?;
        check_interrupted()?;
        let root_path = Arc::new(source_root_path.to_owned());

        let mut r = Vec::new();
        for package_metadata in &metadata.workspace_packages() {
            debug!(
                name = ?package_metadata.name,
                manifest_path = ?package_metadata.manifest_path,
                "Walk package"
            );
            let relative_manifest_path = package_metadata
                .manifest_path
                .strip_prefix(&root_path.as_ref())
                .expect("package manifest should be within source directory")
                .to_owned();
            let package = Package {
                name: package_metadata.name.to_string(),
                version: package_metadata.version.to_string(),
                relative_manifest_path,
            };
            for source_path in direct_package_sources(source_root_path, package_metadata)? {
                check_interrupted()?;
                r.push(Arc::new(SourceFile::new(
                    Arc::clone(&root_path),
                    source_path,
                    package.clone(),
                )?));
            }
        }
        Ok(r)
    }

    fn compose_argv(
        &self,
        build_dir: &BuildDir,
        scenario: &Scenario,
        phase: Phase,
        options: &Options,
    ) -> Result<Vec<String>> {
        let mut cargo_args = vec![cargo_bin(), phase.name().to_string()];
        if phase == Phase::Check || phase == Phase::Build {
            cargo_args.push("--tests".to_string());
        }
        if let Scenario::Mutant(mutant) = scenario {
            let package = &mutant.source_file.package;
            cargo_args.push("--package".to_owned());
            // To cope with trees that indirectly depend on a copy of themselves,
            // as itertools does, build an unambiguous package arg in the form of a URL.
            let mut package_url = url::Url::from_file_path(
                build_dir.path().join(
                    package
                        .relative_manifest_path
                        .parent()
                        .expect("package manifest has a parent"),
                ),
            )
            .expect("make url from path");
            package_url.set_fragment(Some(&format!("{}@{}", package.name, package.version)));
            cargo_args.push(package_url.to_string());
        } else {
            cargo_args.push("--workspace".to_string());
        }
        cargo_args.extend(options.additional_cargo_args.iter().cloned());
        if phase == Phase::Test {
            cargo_args.extend(options.additional_cargo_test_args.iter().cloned());
        }
        Ok(cargo_args)
    }

    fn compose_env(
        &self,
        _scenario: &Scenario,
        _phase: Phase,
        _options: &Options,
    ) -> Result<Vec<(String, String)>> {
        Ok(self.env.clone())
    }
}

/// Return the name of the cargo binary.
fn cargo_bin() -> String {
    // When run as a Cargo subcommand, which is the usual/intended case,
    // $CARGO tells us the right way to call back into it, so that we get
    // the matching toolchain etc.
    env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned())
}

/// Return adjusted CARGO_ENCODED_RUSTFLAGS, including any changes to cap-lints.
///
/// This does not currently read config files; it's too complicated.
///
/// See <https://doc.rust-lang.org/cargo/reference/environment-variables.html>
/// <https://doc.rust-lang.org/rustc/lints/levels.html#capping-lints>
fn rustflags() -> String {
    let mut rustflags: Vec<String> = if let Some(rustflags) = env::var_os("CARGO_ENCODED_RUSTFLAGS")
    {
        rustflags
            .to_str()
            .expect("CARGO_ENCODED_RUSTFLAGS is not valid UTF-8")
            .split(|c| c == '\x1f')
            .map(|s| s.to_owned())
            .collect()
    } else if let Some(rustflags) = env::var_os("RUSTFLAGS") {
        rustflags
            .to_str()
            .expect("RUSTFLAGS is not valid UTF-8")
            .split(' ')
            .map(|s| s.to_owned())
            .collect()
    } else {
        // TODO: We could read the config files, but working out the right target and config seems complicated
        // given the information available here.
        // TODO: All matching target.<triple>.rustflags and target.<cfg>.rustflags config entries joined together.
        // TODO: build.rustflags config value.
        Vec::new()
    };
    rustflags.push("--cap-lints=allow".to_owned());
    // debug!("adjusted rustflags: {:?}", rustflags);
    rustflags.join("\x1f")
}

/// Run `cargo locate-project` to find the path of the `Cargo.toml` enclosing this path.
fn locate_cargo_toml(path: &Utf8Path) -> Result<Utf8PathBuf> {
    let cargo_bin = cargo_bin();
    if !path.is_dir() {
        bail!("{} is not a directory", path);
    }
    let argv: Vec<&str> = vec![&cargo_bin, "locate-project"];
    let stdout = get_command_output(&argv, path)
        .with_context(|| format!("run cargo locate-project in {path:?}"))?;
    let val: Value = serde_json::from_str(&stdout).context("parse cargo locate-project output")?;
    let cargo_toml_path: Utf8PathBuf = val["root"]
        .as_str()
        .context("cargo locate-project output has no root: {stdout:?}")?
        .to_owned()
        .into();
    assert!(cargo_toml_path.is_file());
    Ok(cargo_toml_path)
}

/// Find all the files that are named in the `path` of targets in a Cargo manifest that should be tested.
///
/// These are the starting points for discovering source files.
fn direct_package_sources(
    workspace_root: &Utf8Path,
    package_metadata: &cargo_metadata::Package,
) -> Result<Vec<TreeRelativePathBuf>> {
    let mut found = Vec::new();
    let pkg_dir = package_metadata.manifest_path.parent().unwrap();
    for target in &package_metadata.targets {
        if should_mutate_target(target) {
            if let Ok(relpath) = target.src_path.strip_prefix(workspace_root) {
                let relpath = TreeRelativePathBuf::new(relpath.into());
                debug!(
                    "found mutation target {} of kind {:?}",
                    relpath, target.kind
                );
                found.push(relpath);
            } else {
                warn!("{:?} is not in {:?}", target.src_path, pkg_dir);
            }
        } else {
            debug!(
                "skipping target {:?} of kinds {:?}",
                target.name, target.kind
            );
        }
    }
    found.sort();
    found.dedup();
    Ok(found)
}

fn should_mutate_target(target: &cargo_metadata::Target) -> bool {
    target.kind.iter().any(|k| k.ends_with("lib") || k == "bin")
}

#[cfg(test)]
mod test {
    use std::ffi::OsStr;

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn error_opening_outside_of_crate() {
        CargoTool::new().find_root(Utf8Path::new("/")).unwrap_err();
    }

    #[test]
    fn open_subdirectory_of_crate_opens_the_crate() {
        let root = CargoTool::new()
            .find_root(Utf8Path::new("testdata/tree/factorial/src"))
            .expect("open source tree from subdirectory");
        assert!(root.is_dir());
        assert!(root.join("Cargo.toml").is_file());
        assert!(root.join("src/bin/factorial.rs").is_file());
        assert_eq!(root.file_name().unwrap(), OsStr::new("factorial"));
    }
}

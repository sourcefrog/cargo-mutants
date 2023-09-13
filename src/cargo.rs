// Copyright 2021-2023 Martin Pool

//! Run Cargo as a subprocess, including timeouts and propagating signals.

use std::env;
use std::sync::Arc;

#[allow(unused_imports)]
use anyhow::{anyhow, bail, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use serde_json::Value;
use tracing::debug_span;
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

    /// Find the root files for each relevant package in the source tree.
    ///
    /// A source tree might include multiple packages (e.g. in a Cargo workspace),
    /// and each package might have multiple targets (e.g. a bin and lib). Test targets
    /// are excluded here: we run them, but we don't mutate them.
    ///
    /// Each target has one root file, typically but not necessarily called `src/lib.rs`
    /// or `src/main.rs`. This function returns a list of all those files.
    ///
    /// After this, there is one more level of discovery, by walking those root files
    /// to find `mod` statements, and then recursively walking those files to find
    /// all source files.
    fn root_files(&self, source_root_path: &Utf8Path) -> Result<Vec<Arc<SourceFile>>> {
        let cargo_toml_path = source_root_path.join("Cargo.toml");
        debug!(?cargo_toml_path, ?source_root_path, "find root files");
        check_interrupted()?;
        let metadata = cargo_metadata::MetadataCommand::new()
            .manifest_path(&cargo_toml_path)
            .exec()
            .context("run cargo metadata")?;

        let mut r = Vec::new();
        for package_metadata in &metadata.workspace_packages() {
            check_interrupted()?;
            let _span = debug_span!("package", name = %package_metadata.name).entered();
            let manifest_path = &package_metadata.manifest_path;
            debug!(%manifest_path, "walk package");
            let relative_manifest_path = manifest_path
                .strip_prefix(source_root_path)
                .with_context(|| format!(
                    "manifest path {manifest_path} is not within the detected source root path {source_root_path}"
                ))?
                .to_owned();
            let package = Arc::new(Package {
                name: package_metadata.name.clone(),
                relative_manifest_path,
            });
            for source_path in direct_package_sources(source_root_path, package_metadata)? {
                check_interrupted()?;
                r.push(Arc::new(SourceFile::new(
                    source_root_path,
                    source_path,
                    &package,
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
        Ok(cargo_argv(
            build_dir.path(),
            scenario.package(),
            phase,
            options,
        ))
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

/// Make up the argv for a cargo check/build/test invocation, including argv[0] as the
/// cargo binary itself.
fn cargo_argv(
    build_dir: &Utf8Path,
    package: Option<&Package>,
    phase: Phase,
    options: &Options,
) -> Vec<String> {
    let mut cargo_args = vec![cargo_bin(), phase.name().to_string()];
    if phase == Phase::Check || phase == Phase::Build {
        cargo_args.push("--tests".to_string());
    }
    if let Some(package) = package {
        cargo_args.push("--manifest-path".to_owned());
        cargo_args.push(build_dir.join(&package.relative_manifest_path).to_string());
    } else {
        cargo_args.push("--workspace".to_string());
    }
    cargo_args.extend(options.additional_cargo_args.iter().cloned());
    if phase == Phase::Test {
        cargo_args.extend(options.additional_cargo_test_args.iter().cloned());
    }
    cargo_args
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

    use crate::{Options, Phase};

    use super::*;

    #[test]
    fn generate_cargo_args_for_baseline_with_default_options() {
        let options = Options::default();
        let build_dir = Utf8Path::new("/tmp/buildXYZ");
        assert_eq!(
            cargo_argv(build_dir, None, Phase::Check, &options)[1..],
            ["check", "--tests", "--workspace"]
        );
        assert_eq!(
            cargo_argv(build_dir, None, Phase::Build, &options)[1..],
            ["build", "--tests", "--workspace"]
        );
        assert_eq!(
            cargo_argv(build_dir, None, Phase::Test, &options)[1..],
            ["test", "--workspace"]
        );
    }

    #[test]
    fn generate_cargo_args_with_additional_cargo_test_args_and_package() {
        let mut options = Options::default();
        let package_name = "cargo-mutants-testdata-something";
        let build_dir = Utf8Path::new("/tmp/buildXYZ");
        let relative_manifest_path = Utf8PathBuf::from("testdata/something/Cargo.toml");
        options
            .additional_cargo_test_args
            .extend(["--lib", "--no-fail-fast"].iter().map(|s| s.to_string()));
        let package = Arc::new(Package {
            name: package_name.to_owned(),
            relative_manifest_path: relative_manifest_path.clone(),
        });
        let build_manifest_path = build_dir.join(relative_manifest_path);
        assert_eq!(
            cargo_argv(build_dir, Some(&package), Phase::Check, &options)[1..],
            [
                "check",
                "--tests",
                "--manifest-path",
                build_manifest_path.as_str(),
            ]
        );
        assert_eq!(
            cargo_argv(build_dir, Some(&package), Phase::Build, &options)[1..],
            [
                "build",
                "--tests",
                "--manifest-path",
                build_manifest_path.as_str(),
            ]
        );
        assert_eq!(
            cargo_argv(build_dir, Some(&package), Phase::Test, &options)[1..],
            [
                "test",
                "--manifest-path",
                build_manifest_path.as_str(),
                "--lib",
                "--no-fail-fast"
            ]
        );
    }

    #[test]
    fn generate_cargo_args_with_additional_cargo_args_and_test_args() {
        let mut options = Options::default();
        let build_dir = Utf8Path::new("/tmp/buildXYZ");
        options
            .additional_cargo_test_args
            .extend(["--lib", "--no-fail-fast"].iter().map(|s| s.to_string()));
        options
            .additional_cargo_args
            .extend(["--release".to_owned()]);
        assert_eq!(
            cargo_argv(build_dir, None, Phase::Check, &options)[1..],
            ["check", "--tests", "--workspace", "--release"]
        );
        assert_eq!(
            cargo_argv(build_dir, None, Phase::Build, &options)[1..],
            ["build", "--tests", "--workspace", "--release"]
        );
        assert_eq!(
            cargo_argv(build_dir, None, Phase::Test, &options)[1..],
            [
                "test",
                "--workspace",
                "--release",
                "--lib",
                "--no-fail-fast"
            ]
        );
    }

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

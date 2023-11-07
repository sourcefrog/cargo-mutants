// Copyright 2021-2023 Martin Pool

//! Run Cargo as a subprocess, including timeouts and propagating signals.

use std::env;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, ensure, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::Metadata;
use itertools::Itertools;
use serde_json::Value;
use tracing::debug_span;
#[allow(unused_imports)]
use tracing::{debug, error, info, span, trace, warn, Level};

use crate::outcome::PhaseResult;
use crate::package::Package;
use crate::process::{get_command_output, Process};
use crate::*;

/// Run cargo build, check, or test.
pub fn run_cargo(
    build_dir: &BuildDir,
    packages: Option<&[&Package]>,
    phase: Phase,
    timeout: Duration,
    log_file: &mut LogFile,
    options: &Options,
    console: &Console,
) -> Result<PhaseResult> {
    let _span = debug_span!("run", ?phase).entered();
    let start = Instant::now();
    let argv = cargo_argv(build_dir.path(), packages, phase, options);
    let env = vec![
        ("CARGO_ENCODED_RUSTFLAGS".to_owned(), rustflags()),
        // The tests might use Insta <https://insta.rs>, and we don't want it to write
        // updates to the source tree, and we *certainly* don't want it to write
        // updates and then let the test pass.
        ("INSTA_UPDATE".to_owned(), "no".to_owned()),
    ];
    let process_status = Process::run(&argv, &env, build_dir.path(), timeout, log_file, console)?;
    check_interrupted()?;
    debug!(?process_status, elapsed = ?start.elapsed());
    Ok(PhaseResult {
        phase,
        duration: start.elapsed(),
        process_status,
        argv,
    })
}

pub fn run_cargo_metadata(workspace_dir: &Utf8Path) -> Result<Metadata> {
    let cargo_toml_path = workspace_dir.join("Cargo.toml");
    debug!(?cargo_toml_path, ?workspace_dir, "run cargo metadata");
    check_interrupted()?;
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(&cargo_toml_path)
        .exec()
        .context("run cargo metadata")?;
    check_interrupted()?;
    Ok(metadata)
}

/// Return the name of the cargo binary.
pub fn cargo_bin() -> String {
    // When run as a Cargo subcommand, which is the usual/intended case,
    // $CARGO tells us the right way to call back into it, so that we get
    // the matching toolchain etc.
    env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned())
}

/// Make up the argv for a cargo check/build/test invocation, including argv[0] as the
/// cargo binary itself.
// (This is split out so it's easier to test.)
fn cargo_argv(
    build_dir: &Utf8Path,
    packages: Option<&[&Package]>,
    phase: Phase,
    options: &Options,
) -> Vec<String> {
    let mut cargo_args = vec![cargo_bin(), phase.name().to_string()];
    if phase == Phase::Check || phase == Phase::Build {
        cargo_args.push("--tests".to_string());
    }
    if let Some([package]) = packages {
        // Use the unambiguous form for this case; it works better when the same
        // package occurs multiple times in the tree with different versions?
        cargo_args.push("--manifest-path".to_owned());
        cargo_args.push(build_dir.join(&package.relative_manifest_path).to_string());
    } else if let Some(packages) = packages {
        for package in packages.iter().map(|p| p.name.to_owned()).sorted() {
            cargo_args.push("--package".to_owned());
            cargo_args.push(package);
        }
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

#[cfg(test)]
mod test {
    use std::ffi::OsStr;

    use itertools::Itertools;
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
            cargo_argv(build_dir, Some(&[&package]), Phase::Check, &options)[1..],
            [
                "check",
                "--tests",
                "--manifest-path",
                build_manifest_path.as_str(),
            ]
        );
        assert_eq!(
            cargo_argv(build_dir, Some(&[&package]), Phase::Build, &options)[1..],
            [
                "build",
                "--tests",
                "--manifest-path",
                build_manifest_path.as_str(),
            ]
        );
        assert_eq!(
            cargo_argv(build_dir, Some(&[&package]), Phase::Test, &options)[1..],
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
}

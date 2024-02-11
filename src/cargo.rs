// Copyright 2021-2024 Martin Pool

//! Run Cargo as a subprocess, including timeouts and propagating signals.

use std::env;
use std::time::{Duration, Instant};

use anyhow::Result;
use camino::Utf8Path;
use itertools::Itertools;
use nextest_metadata::NextestExitCode;
use tracing::{debug, debug_span, warn};

use crate::options::TestTool;
use crate::outcome::PhaseResult;
use crate::package::Package;
use crate::process::{Process, ProcessStatus};
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
    if options.test_tool == TestTool::Nextest && phase == Phase::Test {
        // Nextest returns detailed exit codes. I think we should still treat any non-zero result as just an
        // error, but we can at least warn if it's unexpected.
        if let ProcessStatus::Failure(code) = process_status {
            // TODO: When we build with `nextest test --no-test` then we should also check build
            // processes.
            if code != NextestExitCode::TEST_RUN_FAILED as u32 {
                warn!(%code, "nextest process exited with unexpected code (not TEST_RUN_FAILED)");
            }
        }
    }
    Ok(PhaseResult {
        phase,
        duration: start.elapsed(),
        process_status,
        argv,
    })
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
    let mut cargo_args = vec![cargo_bin()];
    if phase == Phase::Test {
        match &options.test_tool {
            TestTool::Cargo => cargo_args.push("test".to_string()),
            TestTool::Nextest => {
                cargo_args.push("nextest".to_string());
                cargo_args.push("run".to_string());
            }
        }
    } else {
        cargo_args.push(phase.name().to_string());
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
    let features = &options.features;
    if features.no_default_features {
        cargo_args.push("--no-default-features".to_owned());
    }
    if features.all_features {
        cargo_args.push("--all-features".to_owned());
    }
    cargo_args.extend(
        features
            .features
            .iter()
            .map(|f| format!("--features={}", f)),
    );
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
    use std::sync::Arc;

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

    #[test]
    fn no_default_features_args_passed_to_cargo() {
        let args = Args::try_parse_from(["mutants", "--no-default-features"].as_slice()).unwrap();
        let options = Options::from_args(&args).unwrap();
        let build_dir = Utf8Path::new("/tmp/buildXYZ");
        assert_eq!(
            cargo_argv(build_dir, None, Phase::Check, &options)[1..],
            ["check", "--tests", "--workspace", "--no-default-features"]
        );
    }

    #[test]
    fn all_features_args_passed_to_cargo() {
        let args = Args::try_parse_from(["mutants", "--all-features"].as_slice()).unwrap();
        let options = Options::from_args(&args).unwrap();
        let build_dir = Utf8Path::new("/tmp/buildXYZ");
        assert_eq!(
            cargo_argv(build_dir, None, Phase::Check, &options)[1..],
            ["check", "--tests", "--workspace", "--all-features"]
        );
    }

    #[test]
    fn feature_args_passed_to_cargo() {
        let args = Args::try_parse_from(
            ["mutants", "--features", "foo", "--features", "bar,baz"].as_slice(),
        )
        .unwrap();
        let options = Options::from_args(&args).unwrap();
        let build_dir = Utf8Path::new("/tmp/buildXYZ");
        assert_eq!(
            cargo_argv(build_dir, None, Phase::Check, &options)[1..],
            [
                "check",
                "--tests",
                "--workspace",
                "--features=foo",
                "--features=bar,baz"
            ]
        );
    }
}

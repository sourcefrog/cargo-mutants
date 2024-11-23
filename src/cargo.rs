// Copyright 2021-2024 Martin Pool

//! Run Cargo as a subprocess, including timeouts and propagating signals.

#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

use std::env;
use std::iter::once;
use std::time::{Duration, Instant};

use itertools::Itertools;
use tracing::{debug, debug_span, warn};

use crate::build_dir::BuildDir;
use crate::console::Console;
use crate::interrupt::check_interrupted;
use crate::options::{Options, TestTool};
use crate::outcome::{Phase, PhaseResult};
use crate::output::ScenarioOutput;
use crate::package::PackageSelection;
use crate::process::{Exit, Process};
use crate::Result;

/// Run cargo build, check, or test.
#[allow(clippy::too_many_arguments)] // I agree it's a lot but I'm not sure wrapping in a struct would be better.
pub fn run_cargo(
    build_dir: &BuildDir,
    jobserver: Option<&jobserver::Client>,
    packages: &PackageSelection,
    phase: Phase,
    timeout: Option<Duration>,
    scenario_output: &mut ScenarioOutput,
    options: &Options,
    console: &Console,
) -> Result<PhaseResult> {
    let _span = debug_span!("run", ?phase).entered();
    let start = Instant::now();
    let argv = cargo_argv(packages, phase, options);
    let mut env = vec![
        // The tests might use Insta <https://insta.rs>, and we don't want it to write
        // updates to the source tree, and we *certainly* don't want it to write
        // updates and then let the test pass.
        ("INSTA_UPDATE".to_owned(), "no".to_owned()),
        ("INSTA_FORCE_PASS".to_owned(), "0".to_owned()),
    ];
    if let Some(encoded_rustflags) = encoded_rustflags(options) {
        debug!(?encoded_rustflags);
        env.push(("CARGO_ENCODED_RUSTFLAGS".to_owned(), encoded_rustflags));
    }
    let process_status = Process::run(
        &argv,
        &env,
        build_dir.path(),
        timeout,
        jobserver,
        scenario_output,
        console,
    )?;
    check_interrupted()?;
    debug!(?process_status, elapsed = ?start.elapsed());
    if let Exit::Failure(code) = process_status {
        // 100 "one or more tests failed" from <https://docs.rs/nextest-metadata/latest/nextest_metadata/enum.NextestExitCode.html>;
        // I'm not addind a dependency to just get one integer.
        if argv[1] == "nextest" && code != 100 {
            // Nextest returns detailed exit codes. I think we should still treat any non-zero result as just an
            // error, but we can at least warn if it's unexpected.
            warn!(%code, "nextest process exited with unexpected code (not TEST_RUN_FAILED)");
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
fn cargo_argv(packages: &PackageSelection, phase: Phase, options: &Options) -> Vec<String> {
    let mut cargo_args = vec![cargo_bin()];
    match phase {
        Phase::Test => match &options.test_tool {
            TestTool::Cargo => cargo_args.push("test".to_string()),
            TestTool::Nextest => {
                cargo_args.push("nextest".to_string());
                cargo_args.push("run".to_string());
            }
        },
        Phase::Build => {
            match &options.test_tool {
                TestTool::Cargo => {
                    // These invocations default to the test profile, and might
                    // have other differences? Generally we want to do everything
                    // to make the tests build, but not actually run them.
                    // See <https://github.com/sourcefrog/cargo-mutants/issues/237>.
                    cargo_args.push("test".to_string());
                    cargo_args.push("--no-run".to_string());
                }
                TestTool::Nextest => {
                    cargo_args.push("nextest".to_string());
                    cargo_args.push("run".to_string());
                    cargo_args.push("--no-run".to_string());
                }
            }
        }
        Phase::Check => {
            cargo_args.push("check".to_string());
            cargo_args.push("--tests".to_string());
        }
    }
    if let Some(profile) = &options.profile {
        match options.test_tool {
            TestTool::Cargo => {
                cargo_args.push(format!("--profile={profile}"));
            }
            TestTool::Nextest => {
                cargo_args.push(format!("--cargo-profile={profile}"));
            }
        }
    }
    cargo_args.push("--verbose".to_string());
    // TODO: If there's just one package then look up its manifest path in the
    // workspace and use that instead, because it's less ambiguous when there's
    // multiple different-version packages with the same name in the workspace.
    // (A rare case, but it happens in itertools.)
    // if let Some([package]) = package_names {
    //     // Use the unambiguous form for this case; it works better when the same
    //     // package occurs multiple times in the tree with different versions?
    //     cargo_args.push("--manifest-path".to_owned());
    //     cargo_args.push(build_dir.join(&package.relative_manifest_path).to_string());
    match packages {
        PackageSelection::All => {
            cargo_args.push("--workspace".to_string());
        }
        PackageSelection::Explicit(package_names) => {
            for package in package_names.iter().sorted() {
                cargo_args.push("--package".to_owned());
                cargo_args.push(package.to_string());
            }
        }
    }
    let features = &options.features;
    if features.no_default_features {
        cargo_args.push("--no-default-features".to_owned());
    }
    if features.all_features {
        cargo_args.push("--all-features".to_owned());
    }
    // N.B. it can make sense to have --all-features and also explicit features from non-default packages.`
    cargo_args.extend(features.features.iter().map(|f| format!("--features={f}")));
    cargo_args.extend(options.additional_cargo_args.iter().cloned());
    if phase == Phase::Test {
        cargo_args.extend(options.additional_cargo_test_args.iter().cloned());
    }
    cargo_args
}

/// Return adjusted `CARGO_ENCODED_RUSTFLAGS`, including any changes to cap-lints.
///
/// It seems we have to set this in the environment because Cargo doesn't expose
/// a way to pass it in as an option from all commands?
///
/// This does not currently read config files; it's too complicated.
///
/// See <https://doc.rust-lang.org/cargo/reference/environment-variables.html>
/// <https://doc.rust-lang.org/rustc/lints/levels.html#capping-lints>
fn encoded_rustflags(options: &Options) -> Option<String> {
    let cap_lints_arg = "--cap-lints=warn";
    let separator = "\x1f";
    if !options.cap_lints {
        None
    } else if let Ok(encoded) = env::var("CARGO_ENCODED_RUSTFLAGS") {
        if encoded.is_empty() {
            Some(cap_lints_arg.to_owned())
        } else {
            Some(encoded + separator + cap_lints_arg)
        }
    } else if let Ok(rustflags) = env::var("RUSTFLAGS") {
        if rustflags.is_empty() {
            Some(cap_lints_arg.to_owned())
        } else {
            Some(
                rustflags
                    .split(' ')
                    .filter(|s| !s.is_empty())
                    .chain(once("--cap-lints=warn"))
                    .collect::<Vec<&str>>()
                    .join(separator),
            )
        }
    } else {
        Some(cap_lints_arg.to_owned())
    }
}

#[cfg(test)]
mod test {
    use clap::Parser;
    use pretty_assertions::assert_eq;
    use rusty_fork::rusty_fork_test;

    use crate::Args;

    use super::*;

    #[test]
    fn generate_cargo_args_for_baseline_with_default_options() {
        let options = Options::default();
        assert_eq!(
            cargo_argv(&PackageSelection::All, Phase::Check, &options)[1..],
            ["check", "--tests", "--verbose", "--workspace"]
        );
        assert_eq!(
            cargo_argv(&PackageSelection::All, Phase::Build, &options)[1..],
            ["test", "--no-run", "--verbose", "--workspace"]
        );
        assert_eq!(
            cargo_argv(&PackageSelection::All, Phase::Test, &options)[1..],
            ["test", "--verbose", "--workspace"]
        );
    }

    #[test]
    fn generate_cargo_args_with_additional_cargo_test_args_and_package() {
        let mut options = Options::default();
        let package_name = "cargo-mutants-testdata-something";
        // let relative_manifest_path = Utf8PathBuf::from("testdata/something/Cargo.toml");
        options
            .additional_cargo_test_args
            .extend(["--lib", "--no-fail-fast"].iter().map(ToString::to_string));
        // TODO: It wolud be a bit better to use `--manifest-path` here, to get
        // the fix for <https://github.com/sourcefrog/cargo-mutants/issues/117>
        // but it's temporarily regressed.
        assert_eq!(
            cargo_argv(
                &PackageSelection::explicit([package_name]),
                Phase::Check,
                &options
            )[1..],
            ["check", "--tests", "--verbose", "--package", package_name]
        );

        // let build_manifest_path = build_dir.join(relative_manifest_path);
        // assert_eq!(
        //     cargo_argv(build_dir, Some(&[package_name]), Phase::Check, &options)[1..],
        //     [
        //         "check",
        //         "--tests",
        //         "--verbose",
        //         "--manifest-path",
        //         build_manifest_path.as_str(),
        //     ]
        // );
        // assert_eq!(
        //     cargo_argv(build_dir, Some(&[package_name]), Phase::Build, &options)[1..],
        //     [
        //         "test",
        //         "--no-run",
        //         "--verbose",
        //         "--manifest-path",
        //         build_manifest_path.as_str(),
        //     ]
        // );
        // assert_eq!(
        //     cargo_argv(build_dir, Some(&[package_name]), Phase::Test, &options)[1..],
        //     [
        //         "test",
        //         "--verbose",
        //         "--manifest-path",
        //         build_manifest_path.as_str(),
        //         "--lib",
        //         "--no-fail-fast"
        //     ]
        // );
    }

    #[test]
    fn generate_cargo_args_with_additional_cargo_args_and_test_args() {
        let mut options = Options::default();
        options
            .additional_cargo_test_args
            .extend(["--lib", "--no-fail-fast"].iter().map(|&s| s.to_string()));
        options
            .additional_cargo_args
            .extend(["--release".to_owned()]);
        assert_eq!(
            cargo_argv(&PackageSelection::All, Phase::Check, &options)[1..],
            ["check", "--tests", "--verbose", "--workspace", "--release"]
        );
        assert_eq!(
            cargo_argv(&PackageSelection::All, Phase::Build, &options)[1..],
            ["test", "--no-run", "--verbose", "--workspace", "--release"]
        );
        assert_eq!(
            cargo_argv(&PackageSelection::All, Phase::Test, &options)[1..],
            [
                "test",
                "--verbose",
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
        assert_eq!(
            cargo_argv(&PackageSelection::All, Phase::Check, &options)[1..],
            [
                "check",
                "--tests",
                "--verbose",
                "--workspace",
                "--no-default-features"
            ]
        );
    }

    #[test]
    fn all_features_args_passed_to_cargo() {
        let args = Args::try_parse_from(["mutants", "--all-features"].as_slice()).unwrap();
        let options = Options::from_args(&args).unwrap();
        assert_eq!(
            cargo_argv(&PackageSelection::All, Phase::Check, &options)[1..],
            [
                "check",
                "--tests",
                "--verbose",
                "--workspace",
                "--all-features"
            ]
        );
    }

    #[test]
    fn cap_lints_passed_to_cargo() {
        let args = Args::try_parse_from(["mutants", "--cap-lints=true"].as_slice()).unwrap();
        let options = Options::from_args(&args).unwrap();
        assert_eq!(
            cargo_argv(&PackageSelection::All, Phase::Check, &options)[1..],
            ["check", "--tests", "--verbose", "--workspace",]
        );
    }

    #[test]
    fn feature_args_passed_to_cargo() {
        let args = Args::try_parse_from(
            ["mutants", "--features", "foo", "--features", "bar,baz"].as_slice(),
        )
        .unwrap();
        let options = Options::from_args(&args).unwrap();
        assert_eq!(
            cargo_argv(&PackageSelection::All, Phase::Check, &options)[1..],
            [
                "check",
                "--tests",
                "--verbose",
                "--workspace",
                "--features=foo",
                "--features=bar,baz"
            ]
        );
    }

    #[test]
    fn profile_arg_passed_to_cargo() {
        let args = Args::try_parse_from(["mutants", "--profile", "mutants"].as_slice()).unwrap();
        let options = Options::from_args(&args).unwrap();
        assert_eq!(
            cargo_argv(&PackageSelection::All, Phase::Check, &options)[1..],
            [
                "check",
                "--tests",
                "--profile=mutants",
                "--verbose",
                "--workspace",
            ]
        );
    }

    #[test]
    fn nextest_gets_special_cargo_profile_option() {
        let args = Args::try_parse_from(
            ["mutants", "--test-tool=nextest", "--profile", "mutants"].as_slice(),
        )
        .unwrap();
        let options = Options::from_args(&args).unwrap();
        assert_eq!(
            cargo_argv(&PackageSelection::All, Phase::Build, &options)[1..],
            [
                "nextest",
                "run",
                "--no-run",
                "--cargo-profile=mutants",
                "--verbose",
                "--workspace",
            ]
        );
    }

    rusty_fork_test! {
        #[test]
        fn rustflags_without_cap_lints_and_no_environment_variables() {
            env::remove_var("RUSTFLAGS");
            env::remove_var("CARGO_ENCODED_RUSTFLAGS");
            assert_eq!(
                encoded_rustflags(&Options {
                    ..Default::default()
                }),
                None
            );
        }
        #[test]
        fn rustflags_with_cap_lints_and_no_environment_variables() {
            env::remove_var("RUSTFLAGS");
            env::remove_var("CARGO_ENCODED_RUSTFLAGS");
            assert_eq!(
                encoded_rustflags(&Options {
                    cap_lints: true,
                    ..Default::default()
                }),
                Some("--cap-lints=warn".into())
            );
        }

        // Don't generate an empty argument if the encoded rustflags is empty.
        #[test]
        fn rustflags_with_empty_encoded_rustflags() {
            env::set_var("CARGO_ENCODED_RUSTFLAGS", "");
            assert_eq!(
                encoded_rustflags(&Options {
                    cap_lints: true,
                    ..Default::default()
                }).unwrap(),
                "--cap-lints=warn"
            );
        }

        #[test]
        fn rustflags_added_to_existing_encoded_rustflags() {
            env::set_var("RUSTFLAGS", "--something\x1f--else");
            env::remove_var("CARGO_ENCODED_RUSTFLAGS");
            let options = Options {
                cap_lints: true,
                ..Default::default()
            };
            assert_eq!(encoded_rustflags(&options).unwrap(), "--something\x1f--else\x1f--cap-lints=warn");
        }

        #[test]
        fn rustflags_added_to_existing_rustflags() {
            env::set_var("RUSTFLAGS", "-Dwarnings");
            env::remove_var("CARGO_ENCODED_RUSTFLAGS");
            assert_eq!(encoded_rustflags(&Options {
                cap_lints: true,
                ..Default::default()
            }).unwrap(), "-Dwarnings\x1f--cap-lints=warn");
        }
    }
}

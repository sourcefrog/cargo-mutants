// Copyright 2021, 2022 Martin Pool

//! Run Cargo as a subprocess, including timeouts and propagating signals.

use std::env;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use camino::Utf8Path;
use serde::Serialize;
use serde_json::Value;
use subprocess::{Popen, PopenConfig, Redirection};
use tracing::{debug, info, warn};

use crate::console::Console;
use crate::log_file::LogFile;
use crate::*;

/// How frequently to check if cargo finished.
const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// The result of running a single Cargo command.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
pub enum CargoResult {
    // Note: This is not, for now, a Result, because it seems like there is
    // no clear "normal" success: sometimes a non-zero exit is what we want, etc.
    // They seem to be all on the same level as far as how the caller should respond.
    // However, failing to even start Cargo is simply an Error, and should
    // probably stop the cargo-mutants job.
    /// Cargo was killed by a timeout.
    Timeout,
    /// Cargo exited successfully.
    Success,
    /// Cargo failed for some reason.
    Failure,
    // TODO: Perhaps distinguish different failure codes.
}

impl CargoResult {
    pub fn success(&self) -> bool {
        matches!(self, CargoResult::Success)
    }
}

/// Run one `cargo` subprocess, with a timeout, and with appropriate handling of interrupts.
pub fn run_cargo(
    argv: &[String],
    in_dir: &Utf8Path,
    log_file: &mut LogFile,
    timeout: Duration,
    console: &Console,
) -> Result<CargoResult> {
    let start = Instant::now();

    let mut env = PopenConfig::current_env();
    // See <https://doc.rust-lang.org/cargo/reference/environment-variables.html>
    // <https://doc.rust-lang.org/rustc/lints/levels.html#capping-lints>
    // TODO: Maybe this should append instead of overwriting it...?
    env.push(("RUSTFLAGS".into(), "--cap-lints=allow".into()));

    let message = format!("run {}", argv.join(" "),);
    log_file.message(&message);
    debug!("{}", message);
    let mut child = Popen::create(
        argv,
        PopenConfig {
            stdin: Redirection::None,
            stdout: Redirection::File(log_file.open_append()?),
            stderr: Redirection::Merge,
            cwd: Some(in_dir.as_os_str().to_owned()),
            env: Some(env),
            ..setpgid_on_unix()
        },
    )
    .with_context(|| format!("failed to spawn {}", argv.join(" ")))?;
    let exit_status = loop {
        if start.elapsed() > timeout {
            info!(
                "timeout after {:.3}s, terminating cargo process...\n",
                start.elapsed().as_secs_f32()
            );
            terminate_child(child, log_file)?;
            return Ok(CargoResult::Timeout);
        } else if let Err(e) = check_interrupted() {
            warn!("interrupted: {}", e);
            console.message(&console::style_interrupted());
            terminate_child(child, log_file)?;
            return Err(e);
        } else if let Some(status) = child.wait_timeout(WAIT_POLL_INTERVAL)? {
            break status;
        }
        console.tick();
    };
    let message = format!(
        "cargo result: {:?} in {:.3}s",
        exit_status,
        start.elapsed().as_secs_f64()
    );
    log_file.message(&message);
    debug!("{}", message);
    check_interrupted()?;
    if exit_status.success() {
        Ok(CargoResult::Success)
    } else {
        Ok(CargoResult::Failure)
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
pub fn cargo_argv(package_name: Option<&str>, phase: Phase, options: &Options) -> Vec<String> {
    let mut cargo_args = vec![cargo_bin(), phase.name().to_string()];
    if phase == Phase::Check || phase == Phase::Build {
        cargo_args.push("--tests".to_string());
    }
    if let Some(package_name) = package_name {
        cargo_args.push("--package".to_owned());
        cargo_args.push(package_name.to_owned());
    } else {
        cargo_args.push("--workspace".to_string());
    }
    cargo_args.extend(options.additional_cargo_args.iter().cloned());
    if phase == Phase::Test {
        cargo_args.extend(options.additional_cargo_test_args.iter().cloned());
    }
    cargo_args
}

#[cfg(unix)]
fn terminate_child(mut child: Popen, log_file: &mut LogFile) -> Result<()> {
    use nix::errno::Errno;
    use nix::sys::signal::{killpg, Signal};

    let pid = nix::unistd::Pid::from_raw(child.pid().expect("child has a pid").try_into().unwrap());
    debug!("terminating cargo process {}", pid);
    if let Err(errno) = killpg(pid, Signal::SIGTERM) {
        if errno == Errno::ESRCH {
            // most likely we raced and it's already gone
            return Ok(());
        } else {
            let message = format!("failed to terminate child: {}", errno);
            warn!("{}", message);
            log_file.message(&message);
            return Err(anyhow!(message));
        }
    }
    child
        .wait()
        .context("wait for child after terminating pgroup")?;
    Ok(())
}

#[cfg(not(unix))]
fn terminate_child(mut child: Popen, log_file: &mut LogFile) -> Result<()> {
    debug!("terminating cargo process {child:?}");
    if let Err(e) = child.terminate() {
        // most likely we raced and it's already gone
        let message = format!("failed to terminate child: {}", e);
        warn!("{}", message);
        log_file.message(&message);
        return Err(anyhow!(message));
    }
    child.wait().context("wait for child after kill")?;
    Ok(())
}

#[cfg(unix)]
fn setpgid_on_unix() -> PopenConfig {
    PopenConfig {
        setpgid: true,
        ..Default::default()
    }
}

#[cfg(not(unix))]
fn setpgid_on_unix() -> PopenConfig {
    Default::default()
}

/// Find the path of the Cargo.toml file enclosing the given directory.
///
/// Returns an error if it's not found.
pub fn locate_project(path: &Utf8Path) -> Result<Utf8PathBuf> {
    let cargo_bin = cargo_bin();
    let argv: Vec<&str> = vec![&cargo_bin, "locate-project"];
    let mut child = Popen::create(
        &argv,
        PopenConfig {
            stdin: Redirection::Pipe,
            stdout: Redirection::Pipe,
            stderr: Redirection::Pipe,
            cwd: Some(path.as_os_str().to_owned()),
            ..Default::default()
        },
    )
    .with_context(|| format!("failed to spawn {}", argv.join(" ")))?;
    let (stdout, stderr) = child
        .communicate(Some(""))
        .context("communicate with cargo locate-project")
        .map(|(a, b)| (a.unwrap(), b.unwrap()))?;
    if !child
        .wait()
        .context("wait for cargo locate-project")?
        .success()
        || stdout.is_empty()
    {
        return Err(anyhow!(stderr));
    }
    debug!("locate-project output: {stdout}");
    let val: Value = serde_json::from_str(&stdout).context("parse cargo locate-project output")?;
    let root = &val["root"];
    root.as_str()
        .context("cargo locate-project output has no root: {stdout:?}")?
        .parse()
        .context("parse cargo locate-project output root to path")
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use super::cargo_argv;
    use crate::{Options, Phase};

    #[test]
    fn generate_cargo_args_for_baseline_with_default_options() {
        let options = Options::default();
        assert_eq!(
            cargo_argv(None, Phase::Check, &options)[1..],
            ["check", "--tests", "--workspace"]
        );
        assert_eq!(
            cargo_argv(None, Phase::Build, &options)[1..],
            ["build", "--tests", "--workspace"]
        );
        assert_eq!(
            cargo_argv(None, Phase::Test, &options)[1..],
            ["test", "--workspace"]
        );
    }

    #[test]
    fn generate_cargo_args_with_additional_cargo_test_args_and_package_name() {
        let mut options = Options::default();
        let package_name = "cargo-mutants-testdata-something";
        options
            .additional_cargo_test_args
            .extend(["--lib", "--no-fail-fast"].iter().map(|s| s.to_string()));
        assert_eq!(
            cargo_argv(Some(package_name), Phase::Check, &options)[1..],
            ["check", "--tests", "--package", package_name]
        );
        assert_eq!(
            cargo_argv(Some(package_name), Phase::Build, &options)[1..],
            ["build", "--tests", "--package", package_name]
        );
        assert_eq!(
            cargo_argv(Some(package_name), Phase::Test, &options)[1..],
            ["test", "--package", package_name, "--lib", "--no-fail-fast"]
        );
    }

    #[test]
    fn generate_cargo_args_with_additional_cargo_args_and_test_args() {
        let mut options = Options::default();
        options
            .additional_cargo_test_args
            .extend(["--lib", "--no-fail-fast"].iter().map(|s| s.to_string()));
        options
            .additional_cargo_args
            .extend(["--release".to_owned()]);
        assert_eq!(
            cargo_argv(None, Phase::Check, &options)[1..],
            ["check", "--tests", "--workspace", "--release"]
        );
        assert_eq!(
            cargo_argv(None, Phase::Build, &options)[1..],
            ["build", "--tests", "--workspace", "--release"]
        );
        assert_eq!(
            cargo_argv(None, Phase::Test, &options)[1..],
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

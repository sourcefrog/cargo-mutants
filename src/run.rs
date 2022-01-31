// Copyright 2021, 2022 Martin Pool

//! Run Cargo as a subprocess.

use std::borrow::Cow;
use std::env;

use std::io::Write;
use std::path::Path;
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use subprocess::{Popen, PopenConfig, Redirection};

use crate::console::Activity;
use crate::lab::LOG_MARKER;
use crate::output::LogFile;

// Until we can reliably stop the grandchild test binaries, by killing a process
// group, timeouts are disabled.
const TEST_TIMEOUT: Duration = Duration::MAX; // Duration::from_secs(1);

/// How frequently to check if cargo finished.
const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// The result of running a single Cargo command.
pub enum CargoResult {
    // Note: This is not, for now, a Result, because it seems like there is
    // no clear "normal" success: sometimes a non-zero exit is what we want, etc.
    // They seem to be all on the same level as far as how the caller should respond.
    // However, failing to even start Cargo is simply an Error, and should
    // probably stop the cargo-mutants job.
    Timeout,
    Success,
    Failure,
}

impl CargoResult {
    pub fn success(&self) -> bool {
        matches!(self, CargoResult::Success)
    }
}

pub fn run_cargo(
    cargo_args: &[&str],
    in_dir: &Path,
    activity: &mut Activity,
    log_file: &LogFile,
) -> Result<CargoResult> {
    let start = Instant::now();
    // When run as a Cargo subcommand, which is the usual/intended case,
    // $CARGO tells us the right way to call back into it, so that we get
    // the matching toolchain etc.
    let cargo_bin: Cow<str> = env::var("CARGO")
        .map(Cow::from)
        .unwrap_or(Cow::Borrowed("cargo"));
    let mut out_file = log_file.open_append().context("open log file")?;
    writeln!(
        out_file,
        "\n{} run {} {}",
        LOG_MARKER,
        cargo_bin,
        cargo_args.join(" "),
    )
    .context("write log marker")?;

    let mut argv: Vec<&str> = vec![&cargo_bin];
    argv.extend(cargo_args.iter());
    let mut child = Popen::create(
        &argv,
        PopenConfig {
            stdin: Redirection::None,
            stdout: Redirection::File(out_file.try_clone()?),
            stderr: Redirection::Merge,
            cwd: Some(in_dir.as_os_str().to_owned()),
            ..setpgid_on_unix()
        },
    )
    .with_context(|| format!("failed to spawn {} {}", cargo_bin, cargo_args.join(" ")))?;
    let exit_status = loop {
        if start.elapsed() > TEST_TIMEOUT {
            writeln!(
                out_file,
                "\n{} timeout after {}s, killing cargo process...",
                LOG_MARKER,
                start.elapsed().as_secs_f32()
            )?;
            if let Err(e) = child.kill() {
                // most likely we raced and it's already gone
                writeln!(
                    out_file,
                    "{} failed to kill child after timeout: {}",
                    LOG_MARKER, e
                )?;
            }
            // Give it a bit of time to exit, then keep signalling until it
            // does stop.
            sleep(Duration::from_millis(500));
            child.wait().context("wait for child after kill")?;
            return Ok(CargoResult::Timeout);
        }
        if let Some(status) = child.wait_timeout(WAIT_POLL_INTERVAL)? {
            break status;
        }
        activity.tick();
    };
    let duration = start.elapsed();
    writeln!(
        out_file,
        "\n{} cargo result: {:?} in {:?}",
        LOG_MARKER, exit_status, duration
    )?;
    if exit_status.success() {
        Ok(CargoResult::Success)
    } else {
        Ok(CargoResult::Failure)
    }
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

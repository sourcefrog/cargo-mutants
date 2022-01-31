// Copyright 2021, 2022 Martin Pool

//! Run Cargo as a subprocess.

use std::borrow::Cow;
use std::env;
use std::path::Path;
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use subprocess::{Popen, PopenConfig, Redirection};

use crate::console::Activity;
use crate::log_file::LogFile;
use crate::outcome::CargoResult;

// Until we can reliably stop the grandchild test binaries, by killing a process
// group, timeouts are disabled.
const TEST_TIMEOUT: Duration = Duration::MAX; // Duration::from_secs(1);

/// How frequently to check if cargo finished.
const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(50);

pub fn run_cargo(
    cargo_args: &[&str],
    in_dir: &Path,
    activity: &mut Activity,
    log_file: &mut LogFile,
) -> Result<CargoResult> {
    let start = Instant::now();
    // When run as a Cargo subcommand, which is the usual/intended case,
    // $CARGO tells us the right way to call back into it, so that we get
    // the matching toolchain etc.
    let cargo_bin: Cow<str> = env::var("CARGO")
        .map(Cow::from)
        .unwrap_or(Cow::Borrowed("cargo"));
    log_file.message(&format!("run {} {}", cargo_bin, cargo_args.join(" "),));

    let mut argv: Vec<&str> = vec![&cargo_bin];
    argv.extend(cargo_args.iter());
    let mut child = Popen::create(
        &argv,
        PopenConfig {
            stdin: Redirection::None,
            stdout: Redirection::File(log_file.open_append()?),
            stderr: Redirection::Merge,
            cwd: Some(in_dir.as_os_str().to_owned()),
            ..setpgid_on_unix()
        },
    )
    .with_context(|| format!("failed to spawn {} {}", cargo_bin, cargo_args.join(" ")))?;
    let exit_status = loop {
        if start.elapsed() > TEST_TIMEOUT {
            log_file.message(&format!(
                "timeout after {}s, killing cargo process...",
                start.elapsed().as_secs_f32()
            ));
            // TODO: Maybe terminate rather than kill.
            if let Err(e) = child.kill() {
                // most likely we raced and it's already gone
                log_file.message(&format!("failed to kill child after timeout: {}", e));
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
    log_file.message(&format!(
        "cargo result: {:?} in {:?}",
        exit_status, duration
    ));
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

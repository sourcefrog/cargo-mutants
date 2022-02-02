// Copyright 2021, 2022 Martin Pool

//! Run Cargo as a subprocess, including timeouts and propagating signals.

use std::borrow::Cow;
use std::env;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use subprocess::{Popen, PopenConfig, Redirection};

use crate::console::Activity;
use crate::log_file::LogFile;
use crate::outcome::CargoResult;
use crate::*;

/// How frequently to check if cargo finished.
const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(50);

pub fn run_cargo(
    cargo_args: &[&str],
    in_dir: &Path,
    activity: &mut Activity,
    log_file: &mut LogFile,
    timeout: Duration,
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
        if start.elapsed() > timeout {
            log_file.message(&format!(
                "timeout after {:.3}s, terminating cargo process...\n",
                start.elapsed().as_secs_f32()
            ));
            terminate_child(child, log_file)?;
            return Ok(CargoResult::Timeout);
        } else if was_interrupted() {
            log_file.message("interrupted\n");
            terminate_child(child, log_file)?;
            return Err(anyhow!("interrupted"));
        } else if let Some(status) = child.wait_timeout(WAIT_POLL_INTERVAL)? {
            break status;
        }
        activity.tick();
    };
    log_file.message(&format!(
        "cargo result: {:?} in {:3}s",
        exit_status,
        start.elapsed().as_secs_f64()
    ));
    if was_interrupted() {
        Err(anyhow!("interrupted"))
    } else if exit_status.success() {
        Ok(CargoResult::Success)
    } else {
        Ok(CargoResult::Failure)
    }
}

#[cfg(unix)]
fn terminate_child(mut child: Popen, log_file: &mut LogFile) -> Result<()> {
    use nix::errno::Errno;
    use nix::sys::signal::{killpg, Signal};
    use std::convert::TryInto;

    let pid = nix::unistd::Pid::from_raw(child.pid().expect("child has a pid").try_into().unwrap());
    if let Err(errno) = killpg(pid, Signal::SIGTERM) {
        if errno == Errno::ESRCH {
            // most likely we raced and it's already gone
            return Ok(());
        } else {
            let message = format!("failed to terminate child: {}", errno);
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
    if let Err(e) = child.terminate() {
        // most likely we raced and it's already gone
        let message = format!("failed to terminate child: {}", e);
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

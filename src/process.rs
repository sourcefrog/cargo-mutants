// Copyright 2021, 2022 Martin Pool

//! Manage a subprocess, with polling, timeouts, termination, and so on.
//!
//! This module is above the external `subprocess` crate, but has no
//! knowledge of whether it's running Cargo or potentially something else.
//!
//! On Unix, the subprocess runs as its own process group, so that any
//! grandchild processses are also signalled if it's interrupted.

use std::ffi::OsString;
use std::io::Read;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context};
use camino::Utf8Path;
use subprocess::{Popen, PopenConfig, Redirection};
#[allow(unused_imports)]
use tracing::{debug, debug_span, error, info, span, trace, warn, Level};

use crate::interrupt::check_interrupted;
use crate::log_file::LogFile;
use crate::Result;

/// How long to wait for metadata-only Cargo commands.
const METADATA_TIMEOUT: Duration = Duration::from_secs(20);

pub struct Process {
    child: Popen,
    start: Instant,
    timeout: Duration,
}

impl Process {
    pub fn start(
        argv: &[String],
        env: &[(&str, &str)],
        cwd: &Utf8Path,
        timeout: Duration,
        log_file: &mut LogFile,
    ) -> Result<Process> {
        let start = Instant::now();
        log_file.message(&format!("run {}", argv.join(" ")));
        debug!("start {argv:?}");
        let mut os_env = PopenConfig::current_env();
        os_env.extend(
            env.iter()
                .map(|&(k, v)| (OsString::from(k), OsString::from(v))),
        );
        let child = Popen::create(
            argv,
            PopenConfig {
                stdin: Redirection::None,
                stdout: Redirection::File(log_file.open_append()?),
                stderr: Redirection::Merge,
                cwd: Some(cwd.as_os_str().to_owned()),
                env: Some(os_env),
                ..setpgid_on_unix()
            },
        )
        .with_context(|| format!("failed to spawn {}", argv.join(" ")))?;
        Ok(Process {
            child,
            start,
            timeout,
        })
    }

    pub fn poll(&mut self) -> Result<Option<ProcessStatus>> {
        let elapsed = self.start.elapsed();
        if elapsed > self.timeout {
            info!(
                "timeout after {:.1}s, terminating child process...",
                elapsed.as_secs_f32()
            );
            self.terminate()?;
            Ok(Some(ProcessStatus::Timeout))
        } else if let Err(e) = check_interrupted() {
            debug!("interrupted, terminating child process...");
            self.terminate()?;
            Err(e)
        } else if let Some(status) = self.child.poll() {
            if status.success() {
                Ok(Some(ProcessStatus::Success))
            } else {
                Ok(Some(ProcessStatus::Failure))
            }
        } else {
            Ok(None)
        }
    }

    /// Terminate the subprocess, initially gently and then harshly.
    ///
    /// Blocks until the subprocess is terminated and then returns the exit status.
    ///
    /// The status might not be Timeout if this raced with a normal exit.
    fn terminate(&mut self) -> Result<()> {
        let _span = span!(Level::DEBUG, "terminate_child", pid = self.child.pid()).entered();
        debug!("terminating child process");
        terminate_child_impl(&mut self.child)?;
        trace!("wait for child after termination");
        if let Some(exit_status) = self
            .child
            .wait_timeout(Duration::from_secs(10))
            .context("wait for child after terminating pgroup")?
        {
            debug!("terminated child exit status {exit_status:?}");
        } else {
            warn!("child did not exit after termination");
            let kill_result = self.child.kill();
            warn!("force kill child: {:?}", kill_result);
            if kill_result.is_ok() {
                if let Ok(Some(exit_status)) = self
                    .child
                    .wait_timeout(Duration::from_secs(10))
                    .context("wait for child after force kill")
                {
                    debug!("force kill child exit status {exit_status:?}");
                } else {
                    warn!("child did not exit after force kill");
                }
            }
        }
        Ok(())
    }
}

#[cfg(unix)]
fn terminate_child_impl(child: &mut Popen) -> Result<()> {
    use nix::errno::Errno;
    use nix::sys::signal::{killpg, Signal};

    let pid = nix::unistd::Pid::from_raw(child.pid().expect("child has a pid").try_into().unwrap());
    if let Err(errno) = killpg(pid, Signal::SIGTERM) {
        // It might have already exited, in which case we can proceed to wait for it.
        if errno != Errno::ESRCH {
            let message = format!("failed to terminate child: {}", errno);
            warn!("{}", message);
            return Err(anyhow!(message));
        }
    }
    Ok(())
}

// We do not yet have a way to mutate this only on Windows, and I mostly test on Unix, so it's just skipped for now.
#[mutants::skip]
#[cfg(not(unix))]
fn terminate_child_impl(child: &mut Popen) -> Result<()> {
    if let Err(e) = child.terminate() {
        // most likely we raced and it's already gone
        let message = format!("failed to terminate child: {}", e);
        warn!("{}", message);
        return Err(anyhow!(message));
    }
    Ok(())
}

/// The result of running a single child process.
#[derive(Debug, PartialEq, Eq)]
pub enum ProcessStatus {
    Success,
    Failure,
    Timeout,
}

#[cfg(unix)]
fn setpgid_on_unix() -> PopenConfig {
    PopenConfig {
        setpgid: true,
        ..Default::default()
    }
}

#[mutants::skip] // Has no effect, so can't be tested.
#[cfg(not(unix))]
fn setpgid_on_unix() -> PopenConfig {
    Default::default()
}

/// Run a command and return its stdout output as a string.
///
/// If the command exits non-zero, the error includes any messages it wrote to stderr.
///
/// The runtime is capped by [METADATA_TIMEOUT].
pub fn get_command_output(argv: &[&str], cwd: &Utf8Path) -> Result<String> {
    // TODO: Perhaps redirect to files so this doesn't jam if there's a lot of output.
    // For the commands we use this for today, which only produce small output, it's OK.
    let _span = debug_span!("get_command_output", argv = ?argv).entered();
    let mut child = Popen::create(
        argv,
        PopenConfig {
            stdin: Redirection::None,
            stdout: Redirection::Pipe,
            stderr: Redirection::Pipe,
            cwd: Some(cwd.as_os_str().to_owned()),
            ..Default::default()
        },
    )
    .with_context(|| format!("failed to spawn {argv:?}"))?;
    match child.wait_timeout(METADATA_TIMEOUT) {
        Err(e) => {
            let message = format!("failed to wait for {argv:?}: {e}");
            return Err(anyhow!(message));
        }
        Ok(None) => {
            let message = format!("{argv:?} timed out",);
            return Err(anyhow!(message));
        }
        Ok(Some(status)) if status.success() => {}
        Ok(Some(status)) => {
            let mut stderr = String::new();
            let _ = child
                .stderr
                .take()
                .expect("child has stderr")
                .read_to_string(&mut stderr);
            error!("child failed with status {status:?}: {stderr}");
            let message = format!("{argv:?} failed with status {status:?}: {stderr}");
            return Err(anyhow!(message));
        }
    }
    let mut stdout = String::new();
    child
        .stdout
        .take()
        .expect("child has stdout")
        .read_to_string(&mut stdout)
        .context("failed to read child stdout")?;
    debug!("output: {}", stdout.trim());
    Ok(stdout)
}

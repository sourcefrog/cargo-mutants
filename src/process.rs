// Copyright 2021-2024 Martin Pool

//! Manage a subprocess, with polling, timeouts, termination, and so on.
//!
//! This module is above the external `subprocess` crate, but has no
//! knowledge of whether it's running Cargo or potentially something else.
//!
//! On Unix, the subprocess runs as its own process group, so that any
//! grandchild processes are also signalled if it's interrupted.

use std::ffi::OsString;
use std::io::Read;
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context};
use camino::Utf8Path;
use fail::fail_point;
use serde::Serialize;
use subprocess::{ExitStatus, Popen, PopenConfig, Redirection};
use tracing::{debug, debug_span, error, span, trace, warn, Level};

use crate::console::Console;
use crate::interrupt::check_interrupted;
use crate::log_file::LogFile;
use crate::Result;

/// How long to wait for metadata-only Cargo commands.
const METADATA_TIMEOUT: Duration = Duration::from_secs(20);

/// How frequently to check if a subprocess finished.
const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(50);

pub struct Process {
    child: Popen,
    start: Instant,
    timeout: Duration,
}

impl Process {
    /// Run a subprocess to completion, watching for interrupts, with a timeout, while
    /// ticking the progress bar.
    pub fn run(
        argv: &[String],
        env: &[(String, String)],
        cwd: &Utf8Path,
        timeout: Duration,
        log_file: &mut LogFile,
        console: &Console,
    ) -> Result<ProcessStatus> {
        fail_point!("Process::run", |r| {
            if let Some(status) = r {
                Ok(ProcessStatus::from_exit_code(
                    status.parse().expect("integer exit code"),
                ))
            } else {
                Err(anyhow!("simulated error in Process::run"))
            }
        });
        let mut child = Process::start(argv, env, cwd, timeout, log_file)?;
        let process_status = loop {
            if let Some(exit_status) = child.poll()? {
                break exit_status;
            } else {
                console.tick();
                sleep(WAIT_POLL_INTERVAL);
            }
        };
        log_file.message(&format!("result: {process_status:?}"));
        Ok(process_status)
    }

    /// Launch a process, and return an object representing the child.
    pub fn start(
        argv: &[String],
        env: &[(String, String)],
        cwd: &Utf8Path,
        timeout: Duration,
        log_file: &mut LogFile,
    ) -> Result<Process> {
        let start = Instant::now();
        let quoted_argv = cheap_shell_quote(argv);
        log_file.message(&quoted_argv);
        debug!(%quoted_argv, "start process");
        let mut os_env = PopenConfig::current_env();
        os_env.extend(
            env.iter()
                .map(|(k, v)| (OsString::from(k), OsString::from(v))),
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

    /// Check if the child process has finished; if so, return its status.
    #[mutants::skip] // It's hard to avoid timeouts if this never works...
    pub fn poll(&mut self) -> Result<Option<ProcessStatus>> {
        let elapsed = self.start.elapsed();
        if elapsed > self.timeout {
            debug!(?elapsed, "timeout, terminating child process...",);
            self.terminate()?;
            Ok(Some(ProcessStatus::Timeout))
        } else if let Err(e) = check_interrupted() {
            debug!("interrupted, terminating child process...");
            self.terminate()?;
            Err(e)
        } else if let Some(status) = self.child.poll() {
            match status {
                _ if status.success() => Ok(Some(ProcessStatus::Success)),
                ExitStatus::Exited(code) => Ok(Some(ProcessStatus::Failure(code))),
                ExitStatus::Signaled(signal) => Ok(Some(ProcessStatus::Signalled(signal))),
                ExitStatus::Undetermined | ExitStatus::Other(_) => Ok(Some(ProcessStatus::Other)),
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
#[allow(unknown_lints, clippy::needless_pass_by_ref_mut)] // To match Windows
fn terminate_child_impl(child: &mut Popen) -> Result<()> {
    use nix::errno::Errno;
    use nix::sys::signal::{killpg, Signal};

    let pid = nix::unistd::Pid::from_raw(child.pid().expect("child has a pid").try_into().unwrap());
    if let Err(errno) = killpg(pid, Signal::SIGTERM) {
        // It might have already exited, in which case we can proceed to wait for it.
        if errno != Errno::ESRCH {
            let message = format!("failed to terminate child: {errno}");
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
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize)]
pub enum ProcessStatus {
    /// Exited with status 0.
    Success,
    /// Exited with status non-0.
    Failure(u32),
    /// Exceeded its timeout, and killed.
    Timeout,
    /// Killed by some signal.
    Signalled(u8),
    /// Unknown or unexpected situation.
    Other,
}

impl ProcessStatus {
    #[allow(dead_code)] // currently only used from failpoint tests, which might not be built
    pub fn from_exit_code(code: u32) -> ProcessStatus {
        if code == 0 {
            ProcessStatus::Success
        } else {
            ProcessStatus::Failure(code)
        }
    }

    pub fn is_success(&self) -> bool {
        *self == ProcessStatus::Success
    }

    pub fn is_timeout(&self) -> bool {
        *self == ProcessStatus::Timeout
    }

    pub fn is_failure(&self) -> bool {
        matches!(self, ProcessStatus::Failure(_))
    }
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

/// Quote an argv slice in Unix shell style.
///
/// This is not completely guaranteed, but is only for debug logs.
fn cheap_shell_quote<S: AsRef<str>, I: IntoIterator<Item = S>>(argv: I) -> String {
    argv.into_iter()
        .map(|s| {
            s.as_ref()
                .chars()
                .flat_map(|c| match c {
                    ' ' | '\t' | '\n' | '\r' | '\\' | '\'' | '"' => vec!['\\', c],
                    _ => vec![c],
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod test {
    use super::cheap_shell_quote;

    #[test]
    fn shell_quoting() {
        assert_eq!(cheap_shell_quote(["foo".to_string()]), "foo");
        assert_eq!(
            cheap_shell_quote(["foo bar", r#"\blah\t"#, r#""quoted""#]),
            r#"foo\ bar \\blah\\t \"quoted\""#
        );
    }
}

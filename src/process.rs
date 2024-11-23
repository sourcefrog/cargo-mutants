// Copyright 2021-2024 Martin Pool

//! Manage a subprocess, with polling, timeouts, termination, and so on.
//!
//! On Unix, the subprocess runs as its own process group, so that any
//! grandchild processes are also signalled if it's interrupted.

#![warn(clippy::pedantic)]
#![allow(clippy::redundant_else)]

use std::ffi::OsStr;
#[cfg(unix)]
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{bail, Context};
use camino::Utf8Path;
use serde::Serialize;
use tracing::{debug, span, trace, warn, Level};

use crate::console::Console;
use crate::interrupt::check_interrupted;
use crate::output::ScenarioOutput;
use crate::Result;

/// How frequently to check if a subprocess finished.
const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(50);

pub struct Process {
    child: Child,
    start: Instant,
    timeout: Option<Duration>,
}

impl Process {
    /// Run a subprocess to completion, watching for interrupts, with a timeout, while
    /// ticking the progress bar.
    pub fn run(
        argv: &[String],
        env: &[(String, String)],
        cwd: &Utf8Path,
        timeout: Option<Duration>,
        jobserver: Option<&jobserver::Client>,
        scenario_output: &mut ScenarioOutput,
        console: &Console,
    ) -> Result<Exit> {
        let mut child = Process::start(argv, env, cwd, timeout, jobserver, scenario_output)?;
        let process_status = loop {
            if let Some(exit_status) = child.poll()? {
                break exit_status;
            }
            console.tick();
            sleep(WAIT_POLL_INTERVAL);
        };
        scenario_output.message(&format!("result: {process_status:?}"))?;
        Ok(process_status)
    }

    /// Launch a process, and return an object representing the child.
    pub fn start(
        argv: &[String],
        env: &[(String, String)],
        cwd: &Utf8Path,
        timeout: Option<Duration>,
        jobserver: Option<&jobserver::Client>,
        scenario_output: &mut ScenarioOutput,
    ) -> Result<Process> {
        let start = Instant::now();
        let quoted_argv = cheap_shell_quote(argv);
        scenario_output.message(&quoted_argv)?;
        debug!(%quoted_argv, "start process");
        let os_env = env.iter().map(|(k, v)| (OsStr::new(k), OsStr::new(v)));
        let mut child = Command::new(&argv[0]);
        child
            .args(&argv[1..])
            .envs(os_env)
            .stdin(Stdio::null())
            .stdout(scenario_output.open_log_append()?)
            .stderr(scenario_output.open_log_append()?)
            .current_dir(cwd);
        if let Some(js) = jobserver {
            js.configure(&mut child);
        }
        #[cfg(unix)]
        child.process_group(0);
        let child = child
            .spawn()
            .with_context(|| format!("failed to spawn {}", argv.join(" ")))?;
        Ok(Process {
            child,
            start,
            timeout,
        })
    }

    /// Check if the child process has finished; if so, return its status.
    #[mutants::skip] // It's hard to avoid timeouts if this never works...
    pub fn poll(&mut self) -> Result<Option<Exit>> {
        if self.timeout.is_some_and(|t| self.start.elapsed() > t) {
            debug!("timeout, terminating child process...",);
            self.terminate()?;
            Ok(Some(Exit::Timeout))
        } else if let Err(e) = check_interrupted() {
            debug!("interrupted, terminating child process...");
            self.terminate()?;
            Err(e)
        } else if let Some(status) = self.child.try_wait()? {
            if let Some(code) = status.code() {
                if code == 0 {
                    return Ok(Some(Exit::Success));
                } else {
                    return Ok(Some(Exit::Failure(
                        code.try_into().context("Read exit code as u32")?,
                    )));
                }
            }
            #[cfg(unix)]
            if let Some(signal) = status.signal() {
                return Ok(Some(Exit::Signalled(
                    signal.try_into().context("Read signal as u8")?,
                )));
            }
            Ok(Some(Exit::Other))
        } else {
            Ok(None)
        }
    }

    /// Terminate the subprocess, initially gently and then harshly.
    ///
    /// Blocks until the subprocess is terminated and then returns the exit status.
    ///
    /// The status might not be Timeout if this raced with a normal exit.
    #[mutants::skip] // would leak processes from tests if skipped
    fn terminate(&mut self) -> Result<()> {
        let _span = span!(Level::DEBUG, "terminate_child", pid = self.child.id()).entered();
        debug!("terminating child process");
        terminate_child_impl(&mut self.child)?;
        trace!("wait for child after termination");
        match self.child.wait() {
            Err(err) => debug!(?err, "Failed to wait for child after termination"),
            Ok(exit) => debug!("terminated child exit status {exit:?}"),
        }
        Ok(())
    }
}

#[cfg(unix)]
#[allow(unknown_lints, clippy::needless_pass_by_ref_mut)] // To match Windows
#[mutants::skip] // hard to exercise the ESRCH edge case
fn terminate_child_impl(child: &mut Child) -> Result<()> {
    use nix::errno::Errno;
    use nix::sys::signal::{killpg, Signal};

    let pid = nix::unistd::Pid::from_raw(child.id().try_into().unwrap());
    match killpg(pid, Signal::SIGTERM) {
        Ok(()) => Ok(()),
        Err(Errno::ESRCH) => {
            Ok(()) // Probably already gone
        }
        Err(Errno::EPERM) if cfg!(target_os = "macos") => {
            Ok(()) // If the process no longer exists then macos can return EPERM (maybe?)
        }
        Err(errno) => {
            let message = format!("failed to terminate child: {errno}");
            warn!("{}", message);
            bail!(message);
        }
    }
}

#[cfg(windows)]
#[mutants::skip] // hard to exercise the ESRCH edge case
fn terminate_child_impl(child: &mut Child) -> Result<()> {
    child.kill().context("Kill child")
}

/// The result of running a single child process.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Exit {
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

impl Exit {
    pub fn is_success(self) -> bool {
        self == Exit::Success
    }

    pub fn is_timeout(self) -> bool {
        self == Exit::Timeout
    }

    pub fn is_failure(self) -> bool {
        matches!(self, Exit::Failure(_))
    }
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
            cheap_shell_quote(["foo bar", r"\blah\t", r#""quoted""#]),
            r#"foo\ bar \\blah\\t \"quoted\""#
        );
    }
}

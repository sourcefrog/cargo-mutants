// Copyright 2021-2024 Martin Pool

//! Manage a subprocess, with polling, timeouts, termination, and so on.
//!
//! On Unix, the subprocess runs as its own process group, so that any
//! grandchild processes are also signalled if it's interrupted.

#![warn(clippy::pedantic)]
#![allow(clippy::redundant_else)]

use std::ffi::OsStr;
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::Context;
use camino::Utf8Path;
use serde::Serialize;
use tracing::{Level, debug, span, trace};

use crate::Result;
use crate::console::Console;
use crate::interrupt::check_interrupted;
use crate::output::ScenarioOutput;

/// How frequently to check if a subprocess finished.
const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(50);

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows::{configure_command, terminate_child};

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix::{configure_command, terminate_child};

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
        let quoted_argv = quote_argv(argv);
        scenario_output.message(&quoted_argv)?;
        debug!(%quoted_argv, "start process");
        let os_env = env.iter().map(|(k, v)| (OsStr::new(k), OsStr::new(v)));
        let mut command = Command::new(&argv[0]);
        command
            .args(&argv[1..])
            .envs(os_env)
            .stdin(Stdio::null())
            .stdout(scenario_output.open_log_append()?)
            .stderr(scenario_output.open_log_append()?)
            .current_dir(cwd);
        if let Some(js) = jobserver {
            js.configure(&mut command);
        }
        configure_command(&mut command);
        let child = command
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
            Ok(Some(status.into()))
        } else {
            Ok(None)
        }
    }

    /// Terminate the subprocess, initially gently and then harshly.
    ///
    /// Blocks until the subprocess is terminated and then returns the exit status.
    ///
    /// The status might not be `Timeout` if this raced with a normal exit.
    #[mutants::skip] // would leak processes from tests if skipped
    fn terminate(&mut self) -> Result<()> {
        let _span = span!(Level::DEBUG, "terminate_child", pid = self.child.id()).entered();
        debug!("terminating child process");
        terminate_child(&mut self.child)?;
        trace!("wait for child after termination");
        match self.child.wait() {
            Err(err) => debug!(?err, "Failed to wait for child after termination"),
            Ok(exit) => debug!("terminated child exit status {exit:?}"),
        }
        Ok(())
    }
}

/// The result of running a single child process.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Exit {
    /// Exited with status 0.
    Success,
    /// Exited with status non-0.
    Failure(i32),
    /// Exceeded its timeout, and killed.
    Timeout,
    /// Killed by some signal.
    #[cfg(unix)]
    Signalled(i32),
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
/// This isn't guaranteed to match the interpretation of a shell or to be safe.
/// It's just for debug logs.
fn quote_argv<S: AsRef<str>, I: IntoIterator<Item = S>>(argv: I) -> String {
    let mut r = String::new();
    for s in argv {
        if !r.is_empty() {
            r.push(' ');
        }
        for c in s.as_ref().chars() {
            match c {
                '\t' => r.push_str(r"\t"),
                '\n' => r.push_str(r"\n"),
                '\r' => r.push_str(r"\r"),
                ' ' | '\\' | '\'' | '"' => {
                    r.push('\\');
                    r.push(c);
                }
                _ => r.push(c),
            }
        }
    }
    r
}

#[cfg(test)]
mod test {
    use super::quote_argv;

    #[test]
    fn shell_quoting() {
        assert_eq!(quote_argv(["foo".to_string()]), "foo");
        assert_eq!(
            quote_argv(["foo bar", r"\blah\x", r#""quoted""#]),
            r#"foo\ bar \\blah\\x \"quoted\""#
        );
        assert_eq!(quote_argv([""]), "");
        assert_eq!(
            quote_argv(["with whitespace", "\r\n\t\t"]),
            r"with\ whitespace \r\n\t\t"
        );
    }
}

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
use itertools::Itertools;
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
use windows::{configure_command, sweep_process_group, terminate_child};

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix::{configure_command, sweep_process_group, terminate_child};

pub mod memory;
use memory::{MemoryLimit, ScenarioMemoryLimit};

/// What sweeping a finished child's process group found and did.
///
/// A scenario's tests can leave processes running: a test binary that was not waited
/// for, or anything a test spawned and forgot. Those processes stay in the child's
/// process group, and would otherwise keep running (and keep allocating) while
/// cargo-mutants moves on to later scenarios.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sweep {
    /// The pids that were in the group, where the platform can enumerate them.
    pub pids: Option<Vec<i32>>,
    /// Whether any process was still in the group after the direct child exited.
    pub strays: bool,
    /// Whether `SIGTERM` was not enough, and `SIGKILL` had to be sent.
    pub killed: bool,
}

impl Sweep {
    /// Describe what was reaped, for an outcome line, or None if nothing was left over.
    pub fn describe(&self) -> Option<String> {
        if !self.strays {
            return None;
        }
        let how = if self.killed { "killed" } else { "reaped" };
        Some(match &self.pids {
            Some(pids) => format!(
                "{how} {n} stray process{es} left over by the tests: {list}",
                n = pids.len(),
                es = if pids.len() == 1 { "" } else { "es" },
                list = pids.iter().join(", ")
            ),
            None => format!("{how} stray processes left over by the tests"),
        })
    }
}

pub struct Process {
    child: Child,
    start: Instant,
    timeout: Option<Duration>,
    /// The memory limit in force for this process tree, if any, held until the process
    /// group has been swept so that its cgroup is empty before we remove it.
    memory: Option<ScenarioMemoryLimit>,
}

impl Process {
    /// Run a subprocess to completion, watching for interrupts, with a timeout, while
    /// ticking the progress bar.
    ///
    /// Whatever the outcome, the child's process group is swept before returning, so
    /// that nothing it left running survives into the next scenario.
    #[allow(clippy::too_many_arguments)] // parallel to run_cargo
    pub fn run(
        argv: &[String],
        env: &[(String, String)],
        cwd: &Utf8Path,
        timeout: Option<Duration>,
        jobserver: Option<&jobserver::Client>,
        memory_limit: Option<&MemoryLimit>,
        scenario_output: &mut ScenarioOutput,
        console: &Console,
    ) -> Result<(Exit, Sweep)> {
        let mut child = Process::start(
            argv,
            env,
            cwd,
            timeout,
            jobserver,
            memory_limit,
            scenario_output,
        )?;
        let result = loop {
            match child.poll() {
                Ok(Some(exit_status)) => break Ok(exit_status),
                Ok(None) => {}
                Err(err) => break Err(err),
            }
            console.tick();
            sleep(WAIT_POLL_INTERVAL);
        };
        let sweep = child.sweep()?;
        // Only safe once the sweep has emptied the cgroup: the kernel won't let us
        // remove a cgroup that still has members.
        if let Some(oom_kills) = child.memory.take().and_then(ScenarioMemoryLimit::finish) {
            debug!(oom_kills, "cgroup memory.events after phase");
        }
        let process_status = result?;
        scenario_output.message(&format!("result: {process_status:?}"))?;
        if let Some(description) = sweep.describe() {
            scenario_output.message(&description)?;
        }
        Ok((process_status, sweep))
    }

    /// Kill anything the child left running in its process group.
    ///
    /// This runs after every phase, not only after a timeout: a scenario that exited
    /// cleanly can still have left a test binary or a process spawned by a test behind.
    fn sweep(&mut self) -> Result<Sweep> {
        let sweep = sweep_process_group(&self.child)?;
        if sweep.strays {
            debug!(
                pids = ?sweep.pids,
                killed = sweep.killed,
                "swept processes left over in the child's process group"
            );
        } else {
            trace!("no processes left in the child's process group");
        }
        Ok(sweep)
    }

    /// Launch a process, and return an object representing the child.
    #[allow(clippy::too_many_arguments)] // parallel to run_cargo
    pub fn start(
        argv: &[String],
        env: &[(String, String)],
        cwd: &Utf8Path,
        timeout: Option<Duration>,
        jobserver: Option<&jobserver::Client>,
        memory_limit: Option<&MemoryLimit>,
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
        let memory = memory_limit.map(MemoryLimit::start).transpose()?;
        if let Some(memory) = &memory {
            memory.configure_command(&mut command)?;
        }
        let child = command
            .spawn()
            .with_context(|| format!("failed to spawn {}", argv.join(" ")))?;
        Ok(Process {
            child,
            start,
            timeout,
            memory,
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

    /// Ask the subprocess to stop, and block until it has.
    ///
    /// This only gets the direct child out of the way so that we can stop waiting on
    /// it; anything else in its process group, including anything that ignored the
    /// `SIGTERM`, is dealt with by the sweep in [`Process::run`].
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

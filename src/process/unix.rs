use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Command, ExitStatus};
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::bail;
use nix::errno::Errno;
use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;
use tracing::warn;

use crate::Result;

use super::{Exit, Sweep};

/// How long to let a process group wind up after `SIGTERM` before sending `SIGKILL`.
const SWEEP_GRACE: Duration = Duration::from_millis(500);

/// How often to check whether a signalled process group has emptied out.
const SWEEP_POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Send a signal to a process group, returning whether the group still had any member.
///
/// A signal of `None` sends nothing and just probes for members.
#[mutants::skip] // hard to exercise the ESRCH edge case
fn signal_group(pgid: Pid, signal: Option<Signal>) -> Result<bool> {
    match killpg(pgid, signal) {
        Ok(()) => Ok(true),
        Err(Errno::ESRCH) => Ok(false), // the group is empty
        Err(Errno::EPERM) if cfg!(target_os = "macos") => {
            Ok(false) // If the process no longer exists then macos can return EPERM (maybe?)
        }
        Err(errno) => {
            // TODO: Maybe strerror?
            let message = format!("failed to signal process group {pgid}: error {errno}");
            warn!("{}", message);
            bail!(message);
        }
    }
}

#[allow(unknown_lints, clippy::needless_pass_by_ref_mut)] // To match Windows
#[mutants::skip] // would leak processes from tests if skipped
pub(super) fn terminate_child(child: &mut Child) -> Result<()> {
    signal_group(child_pgid(child), Some(Signal::SIGTERM))?;
    Ok(())
}

/// Kill anything left in a child's process group once the child itself has exited.
///
/// The child was started as the leader of its own process group, so anything it or its
/// descendants spawned and left running is still in that group, even after being
/// reparented. Send `SIGTERM`, give the group a moment to wind up, then `SIGKILL`
/// whatever is left.
#[mutants::skip] // would leak processes from tests if skipped
pub(super) fn sweep_process_group(child: &Child) -> Result<Sweep> {
    let pgid = child_pgid(child);
    let pids = group_members(pgid);
    if !signal_group(pgid, Some(Signal::SIGTERM))? {
        return Ok(Sweep::default());
    }
    let deadline = Instant::now() + SWEEP_GRACE;
    loop {
        if !signal_group(pgid, None)? {
            return Ok(Sweep {
                pids,
                strays: true,
                killed: false,
            });
        } else if Instant::now() >= deadline {
            break;
        }
        sleep(SWEEP_POLL_INTERVAL);
    }
    signal_group(pgid, Some(Signal::SIGKILL))?;
    Ok(Sweep {
        pids,
        strays: true,
        killed: true,
    })
}

/// The process group id of a child, which (because we start it with `process_group(0)`)
/// is the same as its pid.
fn child_pgid(child: &Child) -> Pid {
    Pid::from_raw(child.id().try_into().expect("child pid fits in pid_t"))
}

/// List the pids currently in a process group.
///
/// Returns `None` on platforms where we can't enumerate processes: the sweep still
/// works there, it just can't say what it killed.
#[cfg(target_os = "linux")]
#[mutants::skip] // only affects what we can say in the debug log
fn group_members(pgid: Pid) -> Option<Vec<i32>> {
    let mut pids = Vec::new();
    for dir_entry in std::fs::read_dir("/proc").ok()?.flatten() {
        let Ok(pid) = dir_entry.file_name().to_string_lossy().parse::<i32>() else {
            continue; // not a process directory
        };
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
            continue; // it exited while we were looking
        };
        // The second field is the command name in parentheses, and may itself contain
        // spaces and parentheses, so only split the fields after its closing paren.
        // Counting from there, the fields are: state, ppid, pgrp.
        let Some(after_comm) = stat.rsplit_once(')').map(|(_, rest)| rest) else {
            continue;
        };
        if after_comm
            .split_whitespace()
            .nth(2)
            .and_then(|field| field.parse::<i32>().ok())
            == Some(pgid.as_raw())
        {
            pids.push(pid);
        }
    }
    Some(pids)
}

#[cfg(not(target_os = "linux"))]
fn group_members(_pgid: Pid) -> Option<Vec<i32>> {
    None
}

#[mutants::skip]
pub(super) fn configure_command(command: &mut Command) {
    command.process_group(0);
}

impl From<ExitStatus> for Exit {
    fn from(status: ExitStatus) -> Self {
        if let Some(code) = status.code() {
            if code == 0 {
                Exit::Success
            } else {
                Exit::Failure(code)
            }
        } else if let Some(signal) = status.signal() {
            Exit::Signalled(signal)
        } else {
            Exit::Other
        }
    }
}

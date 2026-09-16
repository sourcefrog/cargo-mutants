use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Command, ExitStatus};
use std::thread::sleep;
use std::time::Instant;

use anyhow::bail;
use nix::errno::Errno;
use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;
use tracing::warn;

use crate::Result;

use super::{Exit, Sweep, TERM_GRACE, TERM_POLL_INTERVAL};

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

#[allow(unknown_lints, clippy::needless_pass_by_ref_mut)] // To match Windows
#[mutants::skip] // would leak processes from tests if skipped
pub(super) fn kill_child(child: &mut Child) -> Result<()> {
    signal_group(child_pgid(child), Some(Signal::SIGKILL))?;
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
    // Probe before enumerating: almost every phase leaves nothing behind, and listing
    // the group means reading every /proc/<pid>/stat on the machine.
    if !signal_group(pgid, None)? {
        return Ok(Sweep::default());
    }
    let pids = group_members(pgid);
    signal_group(pgid, Some(Signal::SIGTERM))?;
    let deadline = Instant::now() + TERM_GRACE;
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
        sleep(TERM_POLL_INTERVAL);
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
///
/// Callers signal this after the child has been reaped, by which point the group may be
/// empty and the kernel free to hand the same number to something else. Hitting that
/// would take a full wrap of the pid space inside the microseconds between reaping and
/// signalling, so we live with it.
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
    let members = std::fs::read_dir("/proc")
        .ok()?
        .flatten()
        .filter_map(|dir_entry| dir_entry.file_name().to_string_lossy().parse::<i32>().ok())
        .filter(|pid| pgid_of(*pid) == Some(pgid.as_raw()))
        .collect();
    Some(members)
}

/// Read a process's group id out of `/proc/<pid>/stat`.
#[cfg(target_os = "linux")]
#[mutants::skip] // only affects what we can say in the debug log
fn pgid_of(pid: i32) -> Option<i32> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // The second field is the command name in parentheses, and may itself contain spaces
    // and parentheses, so only split the fields after its closing paren. Counting from
    // there, the fields are: state, ppid, pgrp.
    let (_, after_comm) = stat.rsplit_once(')')?;
    after_comm.split_whitespace().nth(2)?.parse().ok()
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

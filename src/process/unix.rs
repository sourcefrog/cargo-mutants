use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Command, ExitStatus};

use anyhow::bail;
use nix::errno::Errno;
use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;
use tracing::warn;

use crate::Result;

use super::Exit;

/// Send a signal to a process group, treating "nothing there" as success.
#[mutants::skip] // hard to exercise the ESRCH edge case
fn signal_group(pgid: Pid, signal: Signal) -> Result<()> {
    match killpg(pgid, signal) {
        Ok(()) => Ok(()),
        Err(Errno::ESRCH) => {
            Ok(()) // Probably already gone
        }
        Err(Errno::EPERM) if cfg!(target_os = "macos") => {
            Ok(()) // If the process no longer exists then macos can return EPERM (maybe?)
        }
        Err(errno) => {
            // TODO: Maybe strerror?
            let message = format!("failed to signal process group {pgid}: error {errno}");
            warn!("{}", message);
            bail!(message);
        }
    }
}

/// The process group id of a child, which (because we start it with `process_group(0)`)
/// is the same as its pid.
fn child_pgid(child: &Child) -> Pid {
    Pid::from_raw(child.id().try_into().expect("child pid fits in pid_t"))
}

#[allow(unknown_lints, clippy::needless_pass_by_ref_mut)] // To match Windows
#[mutants::skip] // hard to exercise the ESRCH edge case
pub(super) fn terminate_child(child: &mut Child) -> Result<()> {
    signal_group(child_pgid(child), Signal::SIGTERM)
}

#[allow(unknown_lints, clippy::needless_pass_by_ref_mut)] // To match Windows
#[mutants::skip] // would leak processes from tests if skipped
pub(super) fn kill_child(child: &mut Child) -> Result<()> {
    signal_group(child_pgid(child), Signal::SIGKILL)
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

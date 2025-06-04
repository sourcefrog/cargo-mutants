use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Command, ExitStatus};

use anyhow::bail;
use nix::errno::Errno;
use nix::sys::signal::{killpg, Signal};
use nix::unistd::Pid;
use tracing::warn;

use crate::Result;

use super::Exit;

#[allow(unknown_lints, clippy::needless_pass_by_ref_mut)] // To match Windows
#[mutants::skip] // hard to exercise the ESRCH edge case
pub(super) fn terminate_child(child: &mut Child) -> Result<()> {
    let pid = Pid::from_raw(child.id().try_into().unwrap());
    match killpg(pid, Signal::SIGTERM) {
        Ok(()) => Ok(()),
        Err(Errno::ESRCH) => {
            Ok(()) // Probably already gone
        }
        Err(Errno::EPERM) if cfg!(target_os = "macos") => {
            Ok(()) // If the process no longer exists then macos can return EPERM (maybe?)
        }
        Err(errno) => {
            // TODO: Maybe strerror?
            let message = format!("failed to terminate child: error {errno}");
            warn!("{}", message);
            bail!(message);
        }
    }
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

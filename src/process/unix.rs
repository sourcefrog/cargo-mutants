use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::Child;

use anyhow::{bail, Context};
use nix::errno::Errno;
use nix::sys::signal::{killpg, Signal};
use nix::unistd::Pid;
use trace::warn;

use crate::Result;

#[allow(unknown_lints, clippy::needless_pass_by_ref_mut)] // To match Windows
#[mutants::skip] // hard to exercise the ESRCH edge case
pub(super) fn terminate_child_impl(child: &mut Child) -> Result<()> {
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

use std::process::{Child, Command, ExitStatus};

use anyhow::Context;

use crate::Result;

use super::{Exit, Sweep};

#[mutants::skip] // hard to exercise the ESRCH edge case
pub(super) fn terminate_child(child: &mut Child) -> Result<()> {
    child.kill().context("Kill child")
}

/// Windows has no process groups; the equivalent would be a job object, which we don't
/// use yet, so there is nothing to sweep.
#[allow(clippy::unnecessary_wraps)] // To match Unix
pub(super) fn sweep_process_group(_child: &Child) -> Result<Sweep> {
    Ok(Sweep::default())
}

#[mutants::skip]
pub(super) fn configure_command(_command: &mut Command) {}

impl From<ExitStatus> for Exit {
    fn from(status: ExitStatus) -> Self {
        if let Some(code) = status.code() {
            if code == 0 {
                Exit::Success
            } else {
                Exit::Failure(code)
            }
        } else {
            Exit::Other
        }
    }
}

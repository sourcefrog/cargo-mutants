use std::process::Child;

use anyhow::Context;

use crate::Result;

#[mutants::skip] // hard to exercise the ESRCH edge case
pub(super) fn terminate_child(child: &mut Child) -> Result<()> {
    child.kill().context("Kill child")
}

#[mutants::skip]
pub(super) fn configure_command(command: &mut Command) {}

pub(super) fn interpret_exit(status: ExitStatus) -> ProcessStatus {
    if let Some(code) = status.code() {
        if code == 0 {
            ProcessStatus::Success
        } else {
            ProcessStatus::Failure(code as u32)
        }
    } else {
        ProcessStatus::Other
    }
}

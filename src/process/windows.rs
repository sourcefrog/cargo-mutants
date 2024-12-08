use std::process::{Child, Command, ExitStatus};

use anyhow::Context;

use crate::Result;

use super::Exit;

#[mutants::skip] // hard to exercise the ESRCH edge case
pub(super) fn terminate_child(child: &mut Child) -> Result<()> {
    child.kill().context("Kill child")
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

use std::process::Child;

use anyhow::Context;

use crate::Result;

#[mutants::skip] // hard to exercise the ESRCH edge case
pub(super) fn terminate_child_impl(child: &mut Child) -> Result<()> {
    child.kill().context("Kill child")
}

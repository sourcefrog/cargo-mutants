use std::fs::FileType;

use anyhow::Context;
use std::path::Path;

use crate::Result;

pub(super) fn copy_symlink(_ft: FileType, src_path: &Path, dest_path: &Path) -> Result<()> {
    let link_target = std::fs::read_link(src_path)
        .with_context(|| format!("Failed to read link {src_path:?}"))?;
    std::os::unix::fs::symlink(link_target, dest_path)
        .with_context(|| format!("Failed to create symlink {dest_path:?}",))?;
    Ok(())
}

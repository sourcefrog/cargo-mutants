use std::fs::FileType;
use std::os::windows::fs::FileTypeExt;
use std::path::Path;

use anyhow::Context;

use crate::Result;
#[mutants::skip] // Mutant tests run on Linux
pub(super) fn copy_symlink(ft: FileType, src_path: &Path, dest_path: &Path) -> Result<()> {
    let link_target =
        std::fs::read_link(src_path).with_context(|| format!("read link {src_path:?}"))?;
    if ft.is_symlink_dir() {
        std::os::windows::fs::symlink_dir(link_target, dest_path)
            .with_context(|| format!("create symlink {dest_path:?}"))?;
    } else if ft.is_symlink_file() {
        std::os::windows::fs::symlink_file(link_target, dest_path)
            .with_context(|| format!("create symlink {dest_path:?}"))?;
    } else {
        anyhow::bail!("Unknown symlink type: {:?}", ft);
    }
    Ok(())
}

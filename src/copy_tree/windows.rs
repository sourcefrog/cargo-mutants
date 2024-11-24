use std::fs::FileType;
use std::os::windows::fs::FileTypeExt;

use anyhow::Context;
use camino::Utf8Path;

use crate::Result;
#[mutants::skip] // Mutant tests run on Linux
fn copy_symlink(ft: FileType, src_path: &Utf8Path, dest_path: &Utf8Path) -> Result<()> {
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

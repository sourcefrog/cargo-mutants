// Copyright 2021 Martin Pool

//! A lab directory in which to test mutations to the source code.

use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{anyhow, Result};
use path_slash::PathExt;
use tempfile::TempDir;

use crate::console;
use crate::source::SourceTree;

/// Holds scratch directories in which files can be mutated and tests executed.
#[derive(Debug)]
pub struct Lab<'s> {
    source: &'s SourceTree,

    /// Top-level temporary directory for this lab.
    tmp: TempDir,

    /// Path (within tmp) holding a copy of the source that can be modified and built.
    build_dir: PathBuf,
}

impl<'s> Lab<'s> {
    pub fn new(source: &'s SourceTree) -> Result<Lab<'s>> {
        let tmp = TempDir::new()?;
        let build_dir = tmp.path().join("build");
        let errs = copy_dir::copy_dir(source.root(), &build_dir)?;
        if !errs.is_empty() {
            eprintln!(
                "error copying source tree {} to {}:",
                &source.root().to_slash_lossy(),
                &build_dir.to_slash_lossy()
            );
            for e in errs {
                eprintln!("  {}", e);
            }
            return Err(anyhow!("error copying source to build directory"));
        }
        Ok(Lab {
            source,
            tmp,
            build_dir,
        })
    }

    /// Test building the unmodified source.
    ///
    /// If there are already-failing tests, proceeding to test mutations
    /// won't give a clear signal.
    pub fn test_clean(&self) -> Result<()> {
        console::show_start("Baseline test with no mutations");
        let output = Command::new("cargo")
            .arg("test")
            .current_dir(&self.build_dir)
            .output()?;
        if output.status.success() {
            console::show_result("ok");
            Ok(())
        } else {
            console::show_result("failed");
            // println!("error: baseline tests in clean tree failed; tests won't continue");
            std::io::stdout().write_all(&output.stdout)?;
            std::io::stdout().write_all(&output.stderr)?;
            Err(anyhow!("build in clean tree failed"))
        }
    }
}

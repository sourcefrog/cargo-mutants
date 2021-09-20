// Copyright 2021 Martin Pool

//! A lab directory in which to test mutations to the source code.

use std::io::{Read, Seek};
use std::path::PathBuf;
use std::process;
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use path_slash::PathExt;
use tempfile::TempDir;

use crate::console;
use crate::mutate::Mutation;
use crate::outcome::{Outcome, Status};
use crate::source::SourceTree;

const TEST_TIMEOUT: Duration = Duration::from_secs(60);

/// Holds scratch directories in which files can be mutated and tests executed.
#[derive(Debug)]
pub struct Lab<'s> {
    source: &'s SourceTree,

    /// Top-level temporary directory for this lab.
    #[allow(unused)] // Needed to set tmpdir lifetime.
    tmp: TempDir,

    /// Path (within tmp) holding a copy of the source that can be modified and built.
    build_dir: PathBuf,
}

impl<'s> Lab<'s> {
    pub fn new(source: &'s SourceTree) -> Result<Lab<'s>> {
        let tmp = TempDir::new()?;
        let build_dir = tmp.path().join("build");
        let start = Instant::now();
        console::show_start("copy source to scratch directory");
        let errs = copy_dir::copy_dir(source.root(), &build_dir)?;
        let duration = start.elapsed();
        if errs.is_empty() {
            console::show_success("done", &duration);
        } else {
            console::show_failure("failed", &duration);
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

    pub fn run(&self) -> Result<()> {
        self.test_clean()?;

        for source_file in self.source.source_files() {
            for mutation in source_file.mutations()? {
                self.test_mutation(&mutation)?;
            }
        }
        Ok(())
    }

    /// Test building the unmodified source.
    ///
    /// If there are already-failing tests, proceeding to test mutations
    /// won't give a clear signal.
    pub fn test_clean(&self) -> Result<()> {
        console::show_start("baseline test with no mutations");
        let outcome = self.run_cargo_test()?;
        console::show_baseline_outcome(&outcome);
        if outcome.status == Status::Passed {
            Ok(())
        } else {
            Err(anyhow!("build in clean tree failed"))
        }
    }

    /// Test with one mutation applied.
    pub fn test_mutation(&self, mutation: &Mutation) -> Result<()> {
        console::show_start(&format!("{}", &mutation));
        // TODO: Maybe an object that reverts on Drop?
        mutation.apply_in_dir(&self.build_dir)?;
        let test_result = self.run_cargo_test();
        // Revert even if there was an error running cargo test
        mutation.revert_in_dir(&self.build_dir)?;
        let outcome = test_result?;
        console::show_outcome(&outcome);
        Ok(())
    }

    fn run_cargo_test(&self) -> Result<Outcome> {
        let start = Instant::now();
        let mut timed_out = false;
        let mut out_file = tempfile::tempfile()?;
        let mut child = Command::new("cargo")
            .arg("test")
            .current_dir(&self.build_dir)
            .stdout(out_file.try_clone()?)
            .stderr(out_file.try_clone()?)
            .stdin(process::Stdio::null())
            .spawn()
            .context("spawn cargo test")?;
        let exit_status = loop {
            if start.elapsed() > TEST_TIMEOUT {
                // eprintln!("bored! killing child...");
                if let Err(e) = child.kill() {
                    // most likely we raced and it's already gone
                    eprintln!("failed to kill child after timeout: {}", e);
                }
                timed_out = true;
                sleep(Duration::from_millis(200));
            }
            match child.try_wait()? {
                Some(status) => break status,
                None => {
                    // eprintln!("wait...");
                    sleep(Duration::from_millis(200))
                }
            }
        };
        out_file.rewind()?;
        let mut out_bytes = Vec::new();
        out_file.read_to_end(&mut out_bytes)?;
        Ok(Outcome {
            status: if timed_out {
                Status::Timeout
            } else {
                exit_status.into()
            },
            stdout: String::from_utf8_lossy(&out_bytes).to_string(),
            stderr: String::new(),
            duration: start.elapsed(),
        })
    }
}

// Copyright 2026 Martin Pool

//! Bound how much memory one scenario's process tree can use.
//!
//! A mutant can turn a bounded loop into an unbounded allocator, and a test process that
//! grows at hundreds of MB/s can exhaust the machine before the test timeout arrives.
//! `--max-memory` puts a ceiling on each scenario instead, so the kernel stops the
//! scenario rather than the machine.
//!
//! Two mechanisms can do this, and they are not equivalent:
//!
//! * cgroup v2 `memory.max`, which limits *resident* memory for the whole process tree
//!   and reports OOM kills through `memory.events`. This is what we want, when we can
//!   get it.
//! * `setrlimit(RLIMIT_AS)`, which limits the *address space* of each process. It is a
//!   cruder proxy -- allocators reserve far more address space than they use -- and it
//!   is only enforced on Linux.

#[cfg(target_os = "linux")]
mod cgroup;

use std::process::Command;

#[cfg(target_os = "linux")]
use anyhow::Context;
use anyhow::bail;
#[cfg(target_os = "linux")]
use tracing::debug;
use tracing::{info, warn};

use crate::Result;

/// How a `--max-memory` limit is applied to a scenario's process tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryMechanism {
    /// A cgroup v2 `memory.max` on a cgroup created for each scenario.
    CgroupV2,
    /// `setrlimit(RLIMIT_AS)` on the cargo process, inherited by everything it spawns.
    RlimitAs,
    /// Nothing on this platform enforces a memory limit, so the option does nothing.
    Unenforced,
}

impl MemoryMechanism {
    fn describe(self) -> &'static str {
        match self {
            MemoryMechanism::CgroupV2 => "cgroup v2 memory.max",
            MemoryMechanism::RlimitAs => "setrlimit(RLIMIT_AS)",
            MemoryMechanism::Unenforced => "no enforced mechanism",
        }
    }
}

/// Choose how to apply `--max-memory`, from what this platform and process can offer.
///
/// `rlimit_settable` says whether `RLIMIT_AS` can be set to the requested limit at all;
/// `rlimit_enforced` says whether the kernel would then act on it, which macOS does not.
///
/// Returns an error, rather than quietly running unlimited, when the user asked for a
/// limit and neither mechanism is available.
pub fn choose_mechanism(
    cgroup_available: bool,
    rlimit_settable: bool,
    rlimit_enforced: bool,
) -> Result<MemoryMechanism> {
    if cgroup_available {
        Ok(MemoryMechanism::CgroupV2)
    } else if rlimit_settable && rlimit_enforced {
        Ok(MemoryMechanism::RlimitAs)
    } else if rlimit_settable {
        Ok(MemoryMechanism::Unenforced)
    } else {
        bail!(
            "--max-memory was requested but no mechanism on this platform can enforce it: \
             cargo-mutants can use cgroup v2 or setrlimit(RLIMIT_AS), and neither is available"
        )
    }
}

/// A per-scenario memory limit, set up once and used for every phase of every scenario.
#[derive(Debug)]
pub struct MemoryLimit {
    bytes: u64,
    mechanism: MemoryMechanism,
    #[cfg(target_os = "linux")]
    cgroups: Option<cgroup::CgroupTree>,
}

impl MemoryLimit {
    /// Set up a limit of `bytes` per scenario, or fail if nothing here can enforce one.
    ///
    /// This is done once, before any mutant is tested, so that an unenforceable limit is
    /// an error the user sees immediately rather than a run that silently had no limit.
    pub fn new(bytes: u64) -> Result<MemoryLimit> {
        #[cfg(target_os = "linux")]
        let cgroups = match cgroup::CgroupTree::probe(bytes) {
            Ok(tree) => Some(tree),
            Err(err) => {
                debug!(?err, "cgroup v2 memory limits are not available");
                None
            }
        };
        #[cfg(target_os = "linux")]
        let cgroup_available = cgroups.is_some();
        #[cfg(not(target_os = "linux"))]
        let cgroup_available = false;

        let mechanism =
            choose_mechanism(cgroup_available, rlimit::settable(bytes), rlimit::ENFORCED)?;
        if mechanism == MemoryMechanism::Unenforced {
            warn!(
                "--max-memory has no effect on this platform: RLIMIT_AS is accepted but not enforced here, and cgroups are not available"
            );
        } else {
            info!(
                "Limiting each scenario to {bytes} bytes of memory using {}",
                mechanism.describe()
            );
        }
        Ok(MemoryLimit {
            bytes,
            mechanism,
            #[cfg(target_os = "linux")]
            cgroups,
        })
    }

    /// Set up the limit for one scenario phase, before its command is spawned.
    #[allow(clippy::unnecessary_wraps)] // fallible only where cgroups exist
    pub fn start(&self) -> Result<ScenarioMemoryLimit> {
        #[cfg(target_os = "linux")]
        let cgroup = self
            .cgroups
            .as_ref()
            .map(|tree| tree.create_scenario(self.bytes))
            .transpose()
            .context("create a cgroup for this scenario")?;
        Ok(ScenarioMemoryLimit {
            bytes: self.bytes,
            mechanism: self.mechanism,
            #[cfg(target_os = "linux")]
            cgroup,
        })
    }
}

/// The memory limit in force for one scenario phase.
#[derive(Debug)]
pub struct ScenarioMemoryLimit {
    bytes: u64,
    mechanism: MemoryMechanism,
    #[cfg(target_os = "linux")]
    cgroup: Option<cgroup::ScenarioCgroup>,
}

impl ScenarioMemoryLimit {
    /// Arrange for the command, once forked, to be subject to the limit, so that it
    /// applies from the very first allocation the child makes.
    pub fn configure_command(&self, command: &mut Command) -> Result<()> {
        match self.mechanism {
            MemoryMechanism::CgroupV2 => self.move_into_cgroup(command),
            MemoryMechanism::RlimitAs => {
                rlimit::apply_to_child(command, self.bytes);
                Ok(())
            }
            MemoryMechanism::Unenforced => Ok(()),
        }
    }

    /// Finish with the limit, returning how many times the kernel OOM-killed something
    /// in this scenario, where the mechanism can tell us.
    #[allow(clippy::unused_self)] // only cgroups have anything to report
    pub fn finish(self) -> Option<u64> {
        #[cfg(target_os = "linux")]
        if let Some(cgroup) = self.cgroup {
            let oom_kills = cgroup.oom_kills();
            cgroup.remove();
            return oom_kills;
        }
        None
    }

    #[cfg(target_os = "linux")]
    fn move_into_cgroup(&self, command: &mut Command) -> Result<()> {
        use std::io::Write;
        use std::os::unix::process::CommandExt;

        let cgroup = self
            .cgroup
            .as_ref()
            .expect("a cgroup was created when the cgroup mechanism was chosen");
        // Open before forking: the child can then move itself in with a single write to
        // an already-open descriptor, which is safe to do between fork and exec.
        let procs = cgroup.open_procs()?;
        // SAFETY: the closure only writes to an already-open file descriptor, and does
        // not allocate or take locks, so it is safe to run between fork and exec.
        unsafe {
            command.pre_exec(move || {
                // "0" means "the process doing the writing".
                (&procs).write_all(b"0\n")
            });
        }
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    #[allow(clippy::unused_self, clippy::unnecessary_wraps)]
    fn move_into_cgroup(&self, _command: &mut Command) -> Result<()> {
        unreachable!("the cgroup mechanism is only ever chosen on Linux")
    }
}

/// `setrlimit(RLIMIT_AS)`, on the platforms where `nix` exposes it and cargo-mutants is
/// supported. Elsewhere there is no rlimit fallback at all, and `--max-memory` is an
/// error unless a cgroup can be used.
#[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
mod rlimit {
    use std::io;
    use std::os::unix::process::CommandExt;
    use std::process::Command;

    use nix::sys::resource::{Resource, getrlimit, setrlimit};
    use tracing::debug;

    /// Whether the kernel acts on `RLIMIT_AS`, as opposed to merely accepting it.
    ///
    /// macOS accepts the call and ignores it, so a limit set there would be a lie.
    pub const ENFORCED: bool = cfg!(any(target_os = "linux", target_os = "android"));

    /// Whether `RLIMIT_AS` can be set to `bytes`: the inherited hard limit is the ceiling
    /// on what an unprivileged process may ask for.
    pub fn settable(bytes: u64) -> bool {
        match getrlimit(Resource::RLIMIT_AS) {
            Ok((_soft, hard)) => hard >= bytes,
            Err(errno) => {
                debug!(?errno, "failed to read RLIMIT_AS");
                false
            }
        }
    }

    /// Limit the address space of the child, and so of everything it goes on to spawn.
    pub fn apply_to_child(command: &mut Command, bytes: u64) {
        // SAFETY: setrlimit is a bare syscall that does not allocate or take locks, so it
        // is safe to call between fork and exec.
        unsafe {
            command.pre_exec(move || {
                setrlimit(Resource::RLIMIT_AS, bytes, bytes).map_err(io::Error::from)
            });
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
mod rlimit {
    use std::process::Command;

    pub const ENFORCED: bool = false;

    pub fn settable(_bytes: u64) -> bool {
        false
    }

    pub fn apply_to_child(_command: &mut Command, _bytes: u64) {
        unreachable!("the RLIMIT_AS mechanism is only ever chosen where it can be set")
    }
}

#[cfg(test)]
mod test {
    use super::{MemoryMechanism, choose_mechanism};

    #[test]
    fn choose_mechanism_prefers_cgroups_over_rlimit() {
        assert_eq!(
            choose_mechanism(true, true, true).unwrap(),
            MemoryMechanism::CgroupV2
        );
        assert_eq!(
            choose_mechanism(false, true, true).unwrap(),
            MemoryMechanism::RlimitAs
        );
    }

    /// On macOS `RLIMIT_AS` can be set but is ignored, so say so rather than pretending.
    #[test]
    fn choose_mechanism_is_unenforced_when_rlimit_is_accepted_but_ignored() {
        assert_eq!(
            choose_mechanism(false, true, false).unwrap(),
            MemoryMechanism::Unenforced
        );
    }

    #[test]
    fn choose_mechanism_with_no_usable_mechanism_is_an_error() {
        let err = choose_mechanism(false, false, false)
            .expect_err("--max-memory with no mechanism should be an error");
        assert!(
            err.to_string().contains("--max-memory"),
            "unhelpful error message: {err}"
        );
        // Also an error if RLIMIT_AS would be enforced but can't be set at all.
        assert!(choose_mechanism(false, false, true).is_err());
    }
}

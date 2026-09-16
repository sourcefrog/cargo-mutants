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
//! * cgroup v2 `memory.max`, which limits *resident* memory for the whole process tree.
//!   This is what we want, when we can get it.
//! * `setrlimit(RLIMIT_AS)`, which limits the *address space* of each process. It is a
//!   cruder proxy -- allocators reserve far more address space than they use -- so the
//!   limit has to be set generously, and it is only enforced on Linux.
//!
//! `any(target_os = "linux", target_os = "android", target_os = "macos")` recurs below:
//! it is where `nix` exposes `RLIMIT_AS` and cargo-mutants is supported. Everywhere else
//! the `RlimitAs` variant does not exist at all, which is what makes it unconstructible
//! rather than merely unreachable.

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
/// `settable` says whether `RLIMIT_AS` can be set to the requested limit at all;
/// `enforced` says whether the kernel would then act on it, which macOS does not.
///
/// Returns an error, rather than quietly running unlimited, when the user asked for a
/// limit and nothing can enforce it.
pub fn choose_mechanism(
    cgroup_available: bool,
    settable: bool,
    enforced: bool,
) -> Result<MemoryMechanism> {
    if cgroup_available {
        Ok(MemoryMechanism::CgroupV2)
    } else if settable && enforced {
        Ok(MemoryMechanism::RlimitAs)
    } else if settable {
        Ok(MemoryMechanism::Unenforced)
    } else {
        bail!(
            "--max-memory was requested but no mechanism on this platform can enforce it: \
             cargo-mutants can use cgroup v2 or setrlimit(RLIMIT_AS), and neither is available"
        )
    }
}

/// A per-scenario memory limit, set up once and used for every phase of every scenario.
///
/// Each variant owns exactly the state its mechanism needs.
#[derive(Debug)]
pub enum MemoryLimit {
    #[cfg(target_os = "linux")]
    CgroupV2 {
        bytes: u64,
        tree: cgroup::CgroupTree,
    },
    #[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
    RlimitAs {
        bytes: u64,
    },
    Unenforced,
}

impl MemoryLimit {
    /// Set up a limit of `bytes` per scenario, or fail if nothing here can enforce one.
    ///
    /// This is done once, before any mutant is tested, so that an unenforceable limit is
    /// an error the user sees immediately rather than a run that silently had no limit.
    pub fn new(bytes: u64) -> Result<MemoryLimit> {
        let limit = MemoryLimit::detect(bytes)?;
        if limit.mechanism() == MemoryMechanism::Unenforced {
            warn!(
                "--max-memory has no effect on this platform: RLIMIT_AS is accepted but not enforced here, and cgroups are not available"
            );
        } else {
            info!(
                "Limiting each scenario to {bytes} bytes of memory using {}",
                limit.mechanism().describe()
            );
        }
        Ok(limit)
    }

    #[cfg(target_os = "linux")]
    fn detect(bytes: u64) -> Result<MemoryLimit> {
        match cgroup::CgroupTree::probe(bytes) {
            Ok(tree) => Ok(MemoryLimit::CgroupV2 { bytes, tree }),
            Err(err) => {
                debug!(?err, "cgroup v2 memory limits are not available");
                MemoryLimit::without_cgroups(bytes)
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn detect(bytes: u64) -> Result<MemoryLimit> {
        MemoryLimit::without_cgroups(bytes)
    }

    /// The limit to fall back to when no cgroup can be used.
    fn without_cgroups(bytes: u64) -> Result<MemoryLimit> {
        match choose_mechanism(false, rlimit::settable(bytes), rlimit::ENFORCED)? {
            #[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
            MemoryMechanism::RlimitAs => Ok(MemoryLimit::RlimitAs { bytes }),
            // We passed `false` for cgroups, so `CgroupV2` cannot come back; folding it in
            // here keeps that a dead branch rather than a panic.
            _ => Ok(MemoryLimit::Unenforced),
        }
    }

    fn mechanism(&self) -> MemoryMechanism {
        match self {
            #[cfg(target_os = "linux")]
            MemoryLimit::CgroupV2 { .. } => MemoryMechanism::CgroupV2,
            #[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
            MemoryLimit::RlimitAs { .. } => MemoryMechanism::RlimitAs,
            MemoryLimit::Unenforced => MemoryMechanism::Unenforced,
        }
    }

    /// Set up the limit for one scenario phase, before its command is spawned.
    #[cfg_attr(not(target_os = "linux"), allow(clippy::unnecessary_wraps))]
    pub fn start(&self) -> Result<ScenarioMemoryLimit> {
        Ok(match self {
            #[cfg(target_os = "linux")]
            MemoryLimit::CgroupV2 { bytes, tree } => ScenarioMemoryLimit::CgroupV2(
                tree.create_scenario(*bytes)
                    .context("create a cgroup for this scenario")?,
            ),
            #[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
            MemoryLimit::RlimitAs { bytes } => ScenarioMemoryLimit::RlimitAs(*bytes),
            MemoryLimit::Unenforced => ScenarioMemoryLimit::Unenforced,
        })
    }
}

/// The memory limit in force for one scenario phase.
///
/// A cgroup is per-scenario state that has to outlive the spawn, which is why this is
/// separate from [`MemoryLimit`]; the cgroup is removed when this is dropped.
#[derive(Debug)]
pub enum ScenarioMemoryLimit {
    #[cfg(target_os = "linux")]
    CgroupV2(cgroup::ScenarioCgroup),
    #[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
    RlimitAs(u64),
    Unenforced,
}

impl ScenarioMemoryLimit {
    /// Arrange for the command, once forked, to be subject to the limit, so that it
    /// applies from the very first allocation the child makes.
    #[cfg_attr(not(target_os = "linux"), allow(clippy::unnecessary_wraps))]
    pub fn configure_command(&self, command: &mut Command) -> Result<()> {
        match self {
            #[cfg(target_os = "linux")]
            ScenarioMemoryLimit::CgroupV2(cgroup) => {
                use std::io::Write;
                use std::os::unix::process::CommandExt;

                let procs = cgroup.open_procs()?;
                // SAFETY: opened before the fork, so the closure only writes to an
                // already-open descriptor -- no allocation, no locks -- which is safe
                // between fork and exec. "0" means "the process doing the writing".
                unsafe {
                    command.pre_exec(move || (&procs).write_all(b"0\n"));
                }
            }
            #[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
            ScenarioMemoryLimit::RlimitAs(bytes) => rlimit::apply_to_child(command, *bytes),
            ScenarioMemoryLimit::Unenforced => {}
        }
        Ok(())
    }
}

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
    pub const ENFORCED: bool = false;

    pub fn settable(_bytes: u64) -> bool {
        false
    }
}

#[cfg(test)]
mod test {
    use super::{MemoryMechanism, choose_mechanism};

    /// The mechanism is picked from what the platform can do, in order of preference.
    #[test]
    fn choose_mechanism_picks_by_availability() {
        // (cgroups available, RLIMIT_AS settable, RLIMIT_AS enforced) -> mechanism, where
        // None means --max-memory should be rejected outright.
        let cases = [
            ((true, true, true), Some(MemoryMechanism::CgroupV2)),
            ((true, false, false), Some(MemoryMechanism::CgroupV2)),
            ((false, true, true), Some(MemoryMechanism::RlimitAs)),
            // macOS: the call is accepted and then ignored.
            ((false, true, false), Some(MemoryMechanism::Unenforced)),
            ((false, false, false), None),
            ((false, false, true), None),
        ];
        for ((cgroup, settable, enforced), expected) in cases {
            assert_eq!(
                choose_mechanism(cgroup, settable, enforced).ok(),
                expected,
                "cgroup={cgroup} settable={settable} enforced={enforced}"
            );
        }
    }

    #[test]
    fn choose_mechanism_with_no_usable_mechanism_names_the_option() {
        let err = choose_mechanism(false, false, false)
            .expect_err("--max-memory with no mechanism should be an error");
        assert!(
            err.to_string().contains("--max-memory"),
            "unhelpful error message: {err}"
        );
    }
}

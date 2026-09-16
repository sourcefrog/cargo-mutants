// Copyright 2026 Martin Pool

//! Bound how much memory one scenario's process tree can use.
//!
//! A mutant can turn a bounded loop into an unbounded allocator, and a test process that
//! grows at hundreds of MB/s can exhaust the machine before the test timeout arrives.
//! `--max-memory` puts a ceiling on each scenario instead, so the kernel stops the
//! scenario rather than the machine.
//!
//! The mechanism here is `setrlimit(RLIMIT_AS)`, which limits the *address space* of each
//! process in the tree. That is a crude proxy for memory use -- allocators reserve far
//! more address space than they ever make resident -- so the limit has to be set
//! generously, and it is only enforced on Linux.
//!
//! `any(target_os = "linux", target_os = "android", target_os = "macos")` recurs below:
//! it is where `nix` exposes `RLIMIT_AS` and cargo-mutants is supported. Everywhere else
//! the `RlimitAs` variant does not exist at all, which is what makes it unconstructible
//! rather than merely unreachable.

use std::process::Command;

use anyhow::bail;
use tracing::{info, warn};

use crate::Result;

/// How a `--max-memory` limit is applied to a scenario's process tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryMechanism {
    /// `setrlimit(RLIMIT_AS)` on the cargo process, inherited by everything it spawns.
    RlimitAs,
    /// Nothing on this platform enforces a memory limit, so the option does nothing.
    Unenforced,
}

impl MemoryMechanism {
    fn describe(self) -> &'static str {
        match self {
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
pub fn choose_mechanism(settable: bool, enforced: bool) -> Result<MemoryMechanism> {
    if settable && enforced {
        Ok(MemoryMechanism::RlimitAs)
    } else if settable {
        Ok(MemoryMechanism::Unenforced)
    } else {
        bail!(
            "--max-memory was requested but no mechanism on this platform can enforce it: \
             cargo-mutants can use setrlimit(RLIMIT_AS), and it is not available"
        )
    }
}

/// A per-scenario memory limit, set up once and used for every phase of every scenario.
///
/// Each variant owns exactly the state its mechanism needs.
#[derive(Debug)]
pub enum MemoryLimit {
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
        let limit = match choose_mechanism(rlimit::settable(bytes), rlimit::ENFORCED)? {
            #[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
            MemoryMechanism::RlimitAs => MemoryLimit::RlimitAs { bytes },
            _ => MemoryLimit::Unenforced,
        };
        if limit.mechanism() == MemoryMechanism::Unenforced {
            warn!(
                "--max-memory has no effect on this platform: RLIMIT_AS is accepted but not enforced here"
            );
        } else {
            info!(
                "Limiting each scenario to {bytes} bytes of memory using {}",
                limit.mechanism().describe()
            );
        }
        Ok(limit)
    }

    fn mechanism(&self) -> MemoryMechanism {
        match self {
            #[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
            MemoryLimit::RlimitAs { .. } => MemoryMechanism::RlimitAs,
            MemoryLimit::Unenforced => MemoryMechanism::Unenforced,
        }
    }

    /// Arrange for the command, once forked, to be subject to the limit, so that it
    /// applies from the very first allocation the child makes.
    pub fn configure_command(&self, command: &mut Command) {
        match self {
            #[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
            MemoryLimit::RlimitAs { bytes } => rlimit::apply_to_child(command, *bytes),
            MemoryLimit::Unenforced => {}
        }
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

    /// The mechanism is picked from what the platform can do.
    #[test]
    fn choose_mechanism_picks_by_availability() {
        // (RLIMIT_AS settable, RLIMIT_AS enforced) -> mechanism, where None means
        // --max-memory should be rejected outright.
        let cases = [
            ((true, true), Some(MemoryMechanism::RlimitAs)),
            // macOS: the call is accepted and then ignored.
            ((true, false), Some(MemoryMechanism::Unenforced)),
            ((false, false), None),
            ((false, true), None),
        ];
        for ((settable, enforced), expected) in cases {
            assert_eq!(
                choose_mechanism(settable, enforced).ok(),
                expected,
                "settable={settable} enforced={enforced}"
            );
        }
    }

    #[test]
    fn choose_mechanism_with_no_usable_mechanism_names_the_option() {
        let err = choose_mechanism(false, false)
            .expect_err("--max-memory with no mechanism should be an error");
        assert!(
            err.to_string().contains("--max-memory"),
            "unhelpful error message: {err}"
        );
    }
}

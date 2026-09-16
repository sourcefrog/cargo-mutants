// Copyright 2026 Martin Pool

//! Per-scenario memory limits using cgroup v2.
//!
//! Each scenario's cargo process tree gets a cgroup of its own with `memory.max` set, so
//! the kernel stops it -- and tells us that it did, through `memory.events` -- rather
//! than letting it eat the machine.
//!
//! The awkward part is finding somewhere to put those cgroups. A cgroup's children only
//! have `memory.max` if the cgroup itself lists `memory` in `cgroup.subtree_control`, and
//! the kernel refuses to set that on a cgroup that contains processes. Since cargo-mutants
//! is itself a process in its own cgroup, we look at, in order of preference:
//!
//! 1. Our own cgroup, if the memory controller is or can be delegated from it: a scenario
//!    cgroup there stays inside whatever limit the operator already put on us.
//! 2. Our parent, if it already delegates the memory controller -- which is exactly the
//!    case when something has already put a `memory.max` fence around us.
//! 3. Our own cgroup again, after moving ourselves down into a leaf so that it no longer
//!    holds any processes. This is a visible side effect, so it's the last resort.

use std::fs::{File, OpenOptions, create_dir, read_to_string, remove_dir, write};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::sleep;
use std::time::Duration;

use anyhow::{Context, bail};
use tracing::{debug, trace};

use crate::Result;

/// Where the unified cgroup v2 hierarchy is mounted.
const CGROUP_ROOT: &str = "/sys/fs/cgroup";

/// How many times, and how far apart, to retry removing a scenario cgroup that the
/// kernel still considers occupied because a killed process has not been reaped yet.
const REMOVE_ATTEMPTS: u32 = 10;
const REMOVE_RETRY_INTERVAL: Duration = Duration::from_millis(20);

/// A cgroup under which one memory-limited cgroup can be made per scenario.
#[derive(Debug)]
pub struct CgroupTree {
    /// A cgroup with the memory controller delegated to its children.
    parent: PathBuf,
}

impl CgroupTree {
    /// Find somewhere to make memory-limited cgroups, or explain why we can't.
    ///
    /// `bytes` is the limit we'll want later: it's applied to a throwaway cgroup here, so
    /// that an unusable hierarchy is a complaint now rather than a failure mid-run.
    pub fn probe(bytes: u64) -> Result<CgroupTree> {
        let own = own_cgroup()?;
        let mut problems: Vec<String> = Vec::new();
        let try_using =
            |dir: &Path, problems: &mut Vec<String>| match CgroupTree::check(dir.to_owned(), bytes)
            {
                Ok(tree) => Some(tree),
                Err(err) => {
                    problems.push(format!("{}: {err:#}", dir.display()));
                    None
                }
            };

        if delegates_memory(&own).unwrap_or(false) || enable_memory_delegation(&own).is_ok() {
            if let Some(tree) = try_using(&own, &mut problems) {
                return Ok(tree);
            }
        } else {
            problems.push(format!(
                "{}: can't delegate the memory controller",
                own.display()
            ));
        }

        if own != Path::new(CGROUP_ROOT)
            && let Some(parent) = own.parent()
            && delegates_memory(parent).unwrap_or(false)
            && let Some(tree) = try_using(parent, &mut problems)
        {
            return Ok(tree);
        }

        // Nothing else worked, so get out of our own cgroup and try it once more.
        match move_self_into_leaf(&own).and_then(|()| enable_memory_delegation(&own)) {
            Ok(()) => {
                if let Some(tree) = try_using(&own, &mut problems) {
                    return Ok(tree);
                }
            }
            Err(err) => problems.push(format!("{}: {err:#}", own.display())),
        }
        bail!("no usable cgroup v2 hierarchy: {}", problems.join("; "))
    }

    /// Prove that a scenario cgroup can really be made here before promising the user one.
    fn check(parent: PathBuf, bytes: u64) -> Result<CgroupTree> {
        let tree = CgroupTree { parent };
        tree.create_scenario(bytes)
            .context("test-create a scenario cgroup")?
            .remove();
        debug!(?tree.parent, "using cgroup v2 for per-scenario memory limits");
        Ok(tree)
    }

    /// Make a cgroup to hold one scenario's process tree, limited to `bytes` of memory.
    pub fn create_scenario(&self, bytes: u64) -> Result<ScenarioCgroup> {
        /// Distinguishes concurrent scenarios from each other.
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let dir = self.parent.join(format!(
            "cargo-mutants-{pid}-{serial}",
            pid = process::id(),
            serial = SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        create_dir(&dir).with_context(|| format!("create cgroup {}", dir.display()))?;
        let cgroup = ScenarioCgroup { dir };
        cgroup.write("memory.max", &bytes.to_string())?;
        // Without this the kernel would swap a runaway scenario out instead of stopping
        // it, which is slower than the failure we're trying to cause.
        cgroup.write("memory.swap.max", "0")?;
        Ok(cgroup)
    }
}

/// The cgroup holding one scenario phase's process tree.
#[derive(Debug)]
pub struct ScenarioCgroup {
    dir: PathBuf,
}

impl ScenarioCgroup {
    /// Open this cgroup's `cgroup.procs`, so that a forked child can move itself in by
    /// writing to an already-open file, without allocating.
    pub fn open_procs(&self) -> Result<File> {
        let path = self.dir.join("cgroup.procs");
        OpenOptions::new()
            .write(true)
            .open(&path)
            .with_context(|| format!("open {}", path.display()))
    }

    /// How many times the kernel OOM-killed a process in this cgroup, from
    /// `memory.events`.
    pub fn oom_kills(&self) -> Option<u64> {
        let events = read_to_string(self.dir.join("memory.events")).ok()?;
        trace!(?self.dir, %events, "cgroup memory.events");
        events
            .lines()
            .find_map(|line| line.strip_prefix("oom_kill "))
            .and_then(|count| count.trim().parse().ok())
    }

    /// Remove the cgroup, which the kernel only allows once it has no members left.
    pub fn remove(self) {
        // The process group sweep has already killed everything in here, but a process
        // that has been killed still counts as a member until it is reaped, so give the
        // kernel a moment to catch up.
        for _ in 0..REMOVE_ATTEMPTS {
            match remove_dir(&self.dir) {
                Ok(()) => return,
                Err(_) => sleep(REMOVE_RETRY_INTERVAL),
            }
        }
        if let Err(err) = remove_dir(&self.dir) {
            // Not worth failing a scenario over: an abandoned empty cgroup costs an inode.
            debug!(?self.dir, ?err, "failed to remove scenario cgroup");
        }
    }

    fn write(&self, name: &str, value: &str) -> Result<()> {
        let path = self.dir.join(name);
        write(&path, value).with_context(|| format!("write {value:?} to {}", path.display()))
    }
}

/// The directory of the cgroup v2 cgroup that this process is in.
fn own_cgroup() -> Result<PathBuf> {
    let root = Path::new(CGROUP_ROOT);
    if !root.join("cgroup.controllers").exists() {
        bail!("{CGROUP_ROOT} is not a cgroup v2 unified hierarchy");
    }
    // The v2 entry in /proc/self/cgroup is the one with hierarchy id 0 and no named
    // controllers; any others are v1 and of no use to us.
    let own = read_to_string("/proc/self/cgroup").context("read /proc/self/cgroup")?;
    let relative = own
        .lines()
        .find_map(|line| line.strip_prefix("0::"))
        .context("no cgroup v2 entry in /proc/self/cgroup")?;
    Ok(root.join(relative.trim().trim_start_matches('/')))
}

/// Whether children of this cgroup get `memory.max`.
fn delegates_memory(dir: &Path) -> Result<bool> {
    let path = dir.join("cgroup.subtree_control");
    Ok(read_to_string(&path)
        .with_context(|| format!("read {}", path.display()))?
        .split_whitespace()
        .any(|controller| controller == "memory"))
}

/// Ask the kernel to give this cgroup's children `memory.max`.
///
/// This fails while the cgroup contains processes, which is the usual case for our own
/// cgroup.
fn enable_memory_delegation(dir: &Path) -> Result<()> {
    let path = dir.join("cgroup.subtree_control");
    write(&path, "+memory").with_context(|| {
        format!(
            "delegate the memory controller by writing to {}",
            path.display()
        )
    })
}

/// Move this process into a child of `dir`, so that `dir` itself holds no processes and
/// can therefore delegate controllers.
fn move_self_into_leaf(dir: &Path) -> Result<()> {
    let leaf = dir.join("cargo-mutants-supervisor");
    if !leaf.exists() {
        create_dir(&leaf).with_context(|| format!("create cgroup {}", leaf.display()))?;
    }
    let procs = leaf.join("cgroup.procs");
    write(&procs, "0\n").with_context(|| format!("move this process into {}", procs.display()))?;
    debug!(?leaf, "moved cargo-mutants into a cgroup of its own");
    Ok(())
}

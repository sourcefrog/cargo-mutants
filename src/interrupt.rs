// Copyright 2022 Martin Pool

//! Handle ctrl-c by setting a global atomic and checking it from long-running
//! operations.

use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::anyhow;

use crate::Result;

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

pub fn install_handler() {
    ctrlc::set_handler(|| INTERRUPTED.store(true, Ordering::SeqCst))
        .expect("install ctrl-c handler");
}

/// Return an error if the program was interrupted and should exit.
pub fn check_interrupted() -> Result<()> {
    if INTERRUPTED.load(Ordering::SeqCst) {
        Err(anyhow!("interrupted"))
    } else {
        Ok(())
    }
}

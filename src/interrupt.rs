// Copyright 2022 Martin Pool

//! Handle ctrl-c etc.

use std::sync::atomic::{AtomicBool, Ordering};

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

pub fn install_handler() {
    ctrlc::set_handler(|| INTERRUPTED.store(true, Ordering::SeqCst))
        .expect("install ctrl-c handler");
}

pub fn was_interrupted() -> bool {
    INTERRUPTED.load(Ordering::SeqCst)
}

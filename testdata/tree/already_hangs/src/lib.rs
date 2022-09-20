//! These tests hang, even in a clean tree.
//!
//! This lets us test we impose a reasonable timeout on clean tree tests.

use std::thread::sleep;
use std::time::Duration;

pub fn infinite_loop() {
    // Not really infinite, so that orphaned processes don't hang around forever.
    // They shouldn't normally happen, but they might when cargo-mutants is itself
    // being mutation tested, or has a bug, etc.
    for i in 0..600 {
        println!("{}", i);
        sleep(Duration::from_secs(1));
    }
}

mod test {
    #[test]
    fn infinite_loop() {
        super::infinite_loop()
    }
}

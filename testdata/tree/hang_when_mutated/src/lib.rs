//! An example of a function that will hang when mutated.
//!
//! An attribute could be added to avoid mutating it, but this tree
//! lets us test the case where that has not yet been fixed.

use std::sync::atomic::{AtomicBool, Ordering};

static TRIGGER: AtomicBool = AtomicBool::new(false);

/// If mutated to return false, the program will spin forever.
fn should_stop() -> bool {
    if TRIGGER.load(Ordering::Relaxed) {
        return true;
    }
    TRIGGER.store(true, Ordering::Relaxed);
    false
}

/// Runs until `should_stop` returns true, and then returns the number
/// of iterations.
pub fn controlled_loop() -> usize {
    for i in 1.. {
        println!("{}", i);
        if should_stop() {
            return i;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    unreachable!();
}

mod test {
    #[test]
    fn controlled_loop_terminates() {
        // Should do two passes: first the trigger is false but gets set,
        // then the trigger is true and the loop terminates.
        assert_eq!(super::controlled_loop(), 2);
    }
}

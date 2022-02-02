//! An example of a function that will hang when mutated.
//!
//! An attribute could be added to avoid mutating it, but this tree
//! lets us test the case where that has not yet been fixed.

/// If mutated to return false, the program will spin forever.
fn should_stop() -> bool {
    true
}

pub fn controlled_loop() {
    for i in 0.. {
        println!("{}", i);
        if should_stop() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

mod test {
    #[test]
    fn controlled_loop_terminates() {
        super::controlled_loop()
    }
}

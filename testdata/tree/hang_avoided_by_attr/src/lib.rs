//! An example of a function we should not mutate because it will hang.

/// If mutated to return false, the program will spin forever.
///
/// Ideally and eventually, cargo-mutants should stop it after a timeout,
/// but that still takes some time, so you can also choose to skip this.
#[mutants::skip]
fn should_stop() -> bool {
    true
}

pub fn controlled_loop() {
    for i in 0.. {
        println!("{}", i);
        if should_stop() {
            break;
        }
    }
}

mod test {
    #[test]
    fn controlled_loop_terminates() {
        super::controlled_loop()
    }
}

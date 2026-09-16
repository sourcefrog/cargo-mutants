//! A tree whose test spawns a background process and then exits successfully.
//!
//! The test records the pid of the background process in the file named by
//! `$BACKGROUND_CHILD_PID_FILE`, so that the cargo-mutants test suite can check that
//! the process group sweep reaps it.

pub fn triple(x: i32) -> i32 {
    x * 3
}

#[cfg(test)]
mod test {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::process::{Command, Stdio};

    /// Spawn a process that outlives this test, and record its pid.
    fn leave_a_process_running() {
        let child = Command::new("sleep")
            .arg("300")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep");
        if let Ok(path) = std::env::var("BACKGROUND_CHILD_PID_FILE") {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .expect("open pid file");
            writeln!(file, "{}", child.id()).expect("write pid");
        }
    }

    #[test]
    fn triple_triples() {
        leave_a_process_running();
        // 3 is chosen so that every mutant of `x * 3` gives a different answer.
        assert_eq!(super::triple(3), 9);
    }
}

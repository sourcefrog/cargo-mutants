#![cfg(unix)]

use std::thread::sleep;
use std::time::Duration;

mod util;
use util::{copy_of_testdata, MAIN_BINARY};

/// If the test hangs and the user (in this case the test suite) interrupts it, then
/// the `cargo test` child should be killed.
///
/// This is a bit hard to directly observe: the property that we really most care
/// about is that _all_ grandchild processes are also killed and nothing is left
/// behind. (On Unix, this is accomplished by use of a pgroup.) However that's a bit
/// hard to mechanically check without reading and interpreting the process tree, which
/// seems likely to be a bit annoying to do portably and without flakes.
/// (But maybe we still should?)
///
/// An easier thing to test is that the cargo-mutants process _thinks_ it has killed
/// the children, and we can observe this in the debug log.
///
/// In this test cargo-mutants has a very long timeout, but the test driver has a
/// short timeout, so it should kill cargo-mutants.
// TODO: An equivalent test on Windows?
#[test]
fn interrupt_caught_and_kills_children() {
    // Test a tree that has enough tests that we'll probably kill it before it completes.

    use std::process::{Command, Stdio};

    use nix::libc::pid_t;
    use nix::sys::signal::{kill, SIGTERM};
    use nix::unistd::Pid;

    let tmp_src_dir = copy_of_testdata("well_tested");
    // We can't use `assert_cmd` `timeout` here because that sends the child a `SIGKILL`,
    // which doesn't give it a chance to clean up. And, `std::process::Command` only
    // has an abrupt kill.

    // Drop RUST_BACKTRACE because the traceback mentions "panic" handler functions
    // and we want to check that the process does not panic.

    // Skip baseline because firstly it should already pass but more importantly
    // #333 exhibited only during non-baseline scenarios.
    let args = [
        MAIN_BINARY.to_str().unwrap(),
        "mutants",
        "--timeout=300",
        "--baseline=skip",
        "--level=trace",
    ];

    println!("Running: {args:?}");
    let mut child = Command::new(args[0])
        .args(&args[1..])
        .current_dir(&tmp_src_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove("RUST_BACKTRACE")
        .spawn()
        .expect("spawn child");

    sleep(Duration::from_secs(2)); // Let it get started
    assert!(
        child.try_wait().expect("try to wait for child").is_none(),
        "child exited early"
    );

    println!("Sending SIGTERM to cargo-mutants...");
    kill(Pid::from_raw(child.id() as pid_t), SIGTERM).expect("send SIGTERM");

    println!("Wait for cargo-mutants to exit...");
    let output = child
        .wait_with_output()
        .expect("wait for child after SIGTERM");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("stdout:\n{stdout}");
    println!("stderr:\n{stderr}");

    assert!(stderr.contains("interrupted"));
    // We used to look here for some other trace messages about how it's interrupted, but
    // that seems to be racy: sometimes the parent sees the child interrupted before it
    // emits these messages? Anyhow, it's not essential.

    // This shouldn't cause a panic though (#333)
    assert!(!stderr.contains("panic"));
    // And we don't want duplicate messages about workers failing.
    assert!(!stderr.contains("Worker thread failed"));
}

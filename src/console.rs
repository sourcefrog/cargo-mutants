// Copyright 2021 Martin Pool

/// Print messages to the terminal.

use std::io::{Write, stdout};

pub fn show_start(msg: &str) {
    print!("{} ... ", msg);
    stdout().flush().unwrap();
}

pub fn show_result(msg: &str) {
    println!("{}", msg);
}

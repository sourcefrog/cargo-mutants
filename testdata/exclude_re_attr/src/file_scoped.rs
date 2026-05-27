//! Sub-module that uses a file-scoped inner `#![mutants::exclude_re(...)]`
//! attribute. The inner attribute applies to every item in this file.
//!
//! Filtered: "replace file_scoped::always_true -> bool with true/false"

#![mutants::exclude_re("replace .* -> bool")]

pub fn always_true() -> bool {
    true
}

pub fn add_three(x: i32) -> i32 {
    x + 3
}

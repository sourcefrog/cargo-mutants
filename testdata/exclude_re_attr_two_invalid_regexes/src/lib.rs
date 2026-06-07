//! Test tree for fail-fast error reporting on multiple malformed
//! `#[mutants::exclude_re("...")]` regexes.
//!
//! Both `(first_invalid` and `(second_invalid` are not valid regexes.
//! cargo-mutants must report the first one to the user and must not
//! emit any reference to the second one, so users always have a single
//! actionable location to fix per run.

#[mutants::exclude_re("(first_invalid")]
pub fn alpha(a: i32, b: i32) -> i32 {
    a + b
}

#[mutants::exclude_re("(second_invalid")]
pub fn beta(a: i32, b: i32) -> i32 {
    a + b
}

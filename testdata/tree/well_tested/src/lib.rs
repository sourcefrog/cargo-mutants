//!  An example tree for `cargo-mutants` with examples of sites where mutants could be, or
//!  shouldn't be, applied.
//!
//! In this well-tested tree:
//!
//! 1. The tests should all pass in a clean tree.
//! 2. Every mutant is caught.

#![allow(unused, dead_code)]

mod inside_mod;
mod nested_function;
mod result;

/// This function is only built for tests so shouldn't be mutated.
#[cfg(test)]
fn outer_test_helper() {
    panic!()
}

fn returns_unit(a: &mut u32) {
    *a += 1;
}

/// Can be mutated to return default (0).
fn returns_42u32() -> u32 {
    42
}

/// Can be mutated to return bool::default.
fn divisible_by_three(a: u32) -> bool {
    a % 3 == 0
}

/// Return `s` repeated twice.
///
/// ```
/// assert_eq!(cargo_mutants_testdata_well_tested::double_string("cat"), "catcat");
/// ```
pub fn double_string(s: &str) -> String {
    let mut r = s.to_owned();
    r.push_str(s);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A test helper function: it shouldn't be mutated because it's inside a
    /// `#[cfg(tests)]` mod.
    fn test_helper() -> usize {
        42
    }

    #[test]
    fn use_test_helper() {
        let result = 2 + test_helper();
        assert_eq!(result, 44);
    }

    #[test]
    fn test_mutatable_functions() {
        assert_eq!(returns_42u32(), 42);

        assert!(divisible_by_three(0));
        assert!(divisible_by_three(9));
        assert!(!divisible_by_three(20));

        let mut a = 0;
        returns_unit(&mut a);
        assert_eq!(a, 1);
    }
}

/// A module that's not only for tests, but should be excluded anyhow.
#[mutants::skip]
mod skip_this_mod {
    fn inside_skipped_mod() {}
}

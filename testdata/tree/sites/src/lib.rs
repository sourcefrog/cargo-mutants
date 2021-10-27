//!  An example tree for `cargo-mutants` with examples of sites where mutants could be, or
//!  shouldn't be, applied.

#![allow(unused, dead_code)]

/// This function is only built for tests so shouldn't be mutated.
#[cfg(test)]
fn outer_test_helper() {}

#[cfg(test)]
mod tests {
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
}

/// A module that's not only for tests, but should be excluded anyhow.
#[mutants::skip]
mod skip_this_mod {
    fn inside_skipped_mod() {}
}

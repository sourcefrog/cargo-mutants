//! An example tree for `cargo-mutants` with examples of sites where mutants could be, or
//! shouldn't be, applied.
//!
//! In this well-tested tree:
//!
//! 1. The tests should all pass in a clean tree.
//! 2. Every mutant is caught.

#![allow(unused, dead_code)]

mod methods;

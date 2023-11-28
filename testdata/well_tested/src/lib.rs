//! An example tree for `cargo-mutants` with examples of sites where mutants could be, or
//! shouldn't be, applied.
//!
//! In this well-tested tree:
//!
//! 1. The tests should all pass in a clean tree.
//! 2. Every mutant is caught.

#![allow(unused, dead_code)]

mod arc;
mod empty_fns;
mod inside_mod;
mod item_mod;
mod methods;
mod nested_function;
mod numbers;
mod result;
mod sets;
pub mod simple_fns;
mod slices;
mod static_item;
mod struct_with_lifetime;

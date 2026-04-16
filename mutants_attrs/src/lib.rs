// Copyright 2021 Martin Pool

//! Attribute macros to control how [cargo-mutants](https://crates.io/crates/cargo-mutants) mutates code.
//!
//! For example, a function that is difficult to test, or has disruptive effects when mutated, can
//! be marked with [macro@skip].
//!

use proc_macro::TokenStream;

/// `cargo mutants` should not mutate functions marked with this attribute.
///
/// This can currently only be applied to functions, not modules or other syntactic constructs.
///
/// ```
/// #[mutants::skip]
/// pub fn some_difficult_function() {
///     // ...
/// }
/// ```
///
/// This is a no-op during compilation, but is seen by cargo-mutants as it processes the source.
#[proc_macro_attribute]
pub fn skip(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Exclude specific mutations matching a regex pattern.
///
/// Unlike [macro@skip], which skips all mutations on a function, this attribute allows
/// you to exclude only mutations whose name matches the given regex, while keeping
/// other mutations active.
///
/// This can be applied to functions, impl blocks, trait blocks, modules, etc.
///
/// ```
/// #[mutants::exclude_re("delete match arm")]
/// pub fn some_function() -> i32 {
///     // ...
///     # 0
/// }
/// ```
///
/// This is a no-op during compilation, but is seen by cargo-mutants as it processes the source.
#[proc_macro_attribute]
pub fn exclude_re(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

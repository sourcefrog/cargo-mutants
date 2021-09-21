// Copyright 2021 Martin Pool

//! Attribute macros to control how enucleate mutates code.

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn skip(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // No compile-time modifications.
    item
}

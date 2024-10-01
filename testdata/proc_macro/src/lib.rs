use std::iter::once;

use proc_macro::{Literal, TokenStream, TokenTree};

/// Count the number of items in a static array.
#[proc_macro]
pub fn static_len(item: TokenStream) -> TokenStream {
    let count = item
        .into_iter()
        .filter(|tt| !matches!(tt, TokenTree::Punct(p) if p.as_char() == ','))
        .count();
    once(TokenTree::Literal(Literal::usize_unsuffixed(count))).collect()
}

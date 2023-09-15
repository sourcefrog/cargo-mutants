// Copyright 2021-2023 Martin Pool

//! Convert a token stream back to (reasonably) pretty Rust code in a string.

use proc_macro2::{Delimiter, TokenTree};
use quote::ToTokens;
use syn::ReturnType;

/// Convert a TokenStream representing some code to a reasonably formatted
/// string of Rust code.
///
/// [TokenStream] has a `to_string`, but it adds spaces in places that don't
/// look idiomatic, so this reimplements it in a way that looks better.
///
/// This is probably not correctly formatted for all Rust syntax, and only tries
/// to cover cases that can emerge from the code we generate.
pub(crate) fn tokens_to_pretty_string<T: ToTokens>(t: T) -> String {
    use TokenTree::*;
    let mut b = String::with_capacity(200);
    let mut ts = t.to_token_stream().into_iter().peekable();
    while let Some(tt) = ts.next() {
        match tt {
            Punct(p) => {
                let pc = p.as_char();
                b.push(pc);
                if ts.peek().is_some() && (b.ends_with("->") || pc == ',' || pc == ';') {
                    b.push(' ');
                }
            }
            Ident(_) | Literal(_) => {
                match tt {
                    Literal(l) => b.push_str(&l.to_string()),
                    Ident(i) => b.push_str(&i.to_string()),
                    _ => unreachable!(),
                };
                if let Some(next) = ts.peek() {
                    match next {
                        Ident(_) | Literal(_) => b.push(' '),
                        Punct(p) => match p.as_char() {
                            ',' | ';' | '<' | '>' | ':' | '.' | '!' => (),
                            _ => b.push(' '),
                        },
                        Group(_) => (),
                    }
                }
            }
            Group(g) => {
                match g.delimiter() {
                    Delimiter::Brace => b.push('{'),
                    Delimiter::Bracket => b.push('['),
                    Delimiter::Parenthesis => b.push('('),
                    Delimiter::None => (),
                }
                b.push_str(&tokens_to_pretty_string(g.stream()));
                match g.delimiter() {
                    Delimiter::Brace => b.push('}'),
                    Delimiter::Bracket => b.push(']'),
                    Delimiter::Parenthesis => b.push(')'),
                    Delimiter::None => (),
                }
            }
        }
    }
    debug_assert!(
        !b.ends_with(' '),
        "generated a trailing space: ts={ts:?}, b={b:?}",
        ts = t.to_token_stream(),
    );
    b
}

pub(crate) fn return_type_to_pretty_string(return_type: &ReturnType) -> String {
    match return_type {
        ReturnType::Default => String::new(),
        ReturnType::Type(arrow, typ) => {
            format!(
                "{} {}",
                arrow.to_token_stream(),
                tokens_to_pretty_string(typ)
            )
        }
    }
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;
    use quote::quote;

    use super::tokens_to_pretty_string;

    #[test]
    fn pretty_format() {
        assert_eq!(
            tokens_to_pretty_string(quote! {
                <impl Iterator for MergeTrees < AE , BE , AIT , BIT > > :: next
                -> Option < Self ::  Item >
            }),
            "<impl Iterator for MergeTrees<AE, BE, AIT, BIT>>::next -> Option<Self::Item>"
        );
        assert_eq!(
            tokens_to_pretty_string(quote! { Lex < 'buf >::take }),
            "Lex<'buf>::take"
        );
    }
}

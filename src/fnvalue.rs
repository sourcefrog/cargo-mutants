// Copyright 2021-2023 Martin Pool

//! Mutations of replacing a function body with a value of a (hopefully) appropriate type.

use std::iter;

use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    AngleBracketedGenericArguments, AssocType, Expr, GenericArgument, Ident, Path, PathArguments,
    ReturnType, TraitBound, Type, TypeArray, TypeImplTrait, TypeParamBound, TypeSlice, TypeTuple,
};
use tracing::trace;

/// Generate replacement text for a function based on its return type.
pub(crate) fn return_type_replacements(
    return_type: &ReturnType,
    error_exprs: &[Expr],
) -> Vec<TokenStream> {
    match return_type {
        ReturnType::Default => vec![quote! { () }],
        ReturnType::Type(_rarrow, type_) => type_replacements(type_, error_exprs).collect_vec(),
    }
}

/// Generate some values that we hope are reasonable replacements for a type.
///
/// This is really the heart of cargo-mutants.
fn type_replacements(type_: &Type, error_exprs: &[Expr]) -> impl Iterator<Item = TokenStream> {
    // This could probably change to run from some configuration rather than
    // hardcoding various types, which would make it easier to support tree-specific
    // mutation values, and perhaps reduce duplication. However, it seems better
    // to support all the core cases with direct code first to learn what generalizations
    // are needed.
    match type_ {
        Type::Path(syn::TypePath { path, .. }) => {
            // dbg!(&path);
            if path.is_ident("bool") {
                vec![quote! { true }, quote! { false }]
            } else if path.is_ident("String") {
                vec![quote! { String::new() }, quote! { "xyzzy".into() }]
            } else if path.is_ident("str") {
                vec![quote! { "" }, quote! { "xyzzy" }]
            } else if path_is_unsigned(path) {
                vec![quote! { 0 }, quote! { 1 }]
            } else if path_is_signed(path) {
                vec![quote! { 0 }, quote! { 1 }, quote! { -1 }]
            } else if path_is_nonzero_signed(path) {
                vec![quote! { 1 }, quote! { -1 }]
            } else if path_is_nonzero_unsigned(path) {
                vec![quote! { 1 }]
            } else if path_is_float(path) {
                vec![quote! { 0.0 }, quote! { 1.0 }, quote! { -1.0 }]
            } else if path_ends_with(path, "Result") {
                if let Some(ok_type) = match_first_type_arg(path, "Result") {
                    type_replacements(ok_type, error_exprs)
                        .map(|rep| {
                            quote! { Ok(#rep) }
                        })
                        .collect_vec()
                } else {
                    // A result with no type arguments, like `fmt::Result`; hopefully
                    // the Ok value can be constructed with Default.
                    vec![quote! { Ok(Default::default()) }]
                }
                .into_iter()
                .chain(error_exprs.iter().map(|error_expr| {
                    quote! { Err(#error_expr) }
                }))
                .collect_vec()
            } else if path_ends_with(path, "HttpResponse") {
                vec![quote! { HttpResponse::Ok().finish() }]
            } else if let Some(some_type) = match_first_type_arg(path, "Option") {
                iter::once(quote! { None })
                    .chain(type_replacements(some_type, error_exprs).map(|rep| {
                        quote! { Some(#rep) }
                    }))
                    .collect_vec()
            } else if let Some(element_type) = match_first_type_arg(path, "Vec") {
                // Generate an empty Vec, and then a one-element vec for every recursive
                // value.
                iter::once(quote! { vec![] })
                    .chain(type_replacements(element_type, error_exprs).map(|rep| {
                        quote! { vec![#rep] }
                    }))
                    .collect_vec()
            } else if let Some(borrowed_type) = match_first_type_arg(path, "Cow") {
                // TODO: We could specialize Cows for cases like Vec and Box where
                // we would have to leak to make the reference; perhaps it would only
                // look better...
                type_replacements(borrowed_type, error_exprs)
                    .flat_map(|rep| {
                        [
                            quote! { Cow::Borrowed(#rep) },
                            quote! { Cow::Owned(#rep.to_owned()) },
                        ]
                    })
                    .collect_vec()
            } else if let Some((container_type, inner_type)) = known_container(path) {
                // Something like Arc, Mutex, etc.
                // TODO: Ideally we should use the path without relying on it being
                // imported, but we must strip or rewrite the arguments, so that
                // `std::sync::Arc<String>` becomes either `std::sync::Arc::<String>::new`
                // or at least `std::sync::Arc::new`. Similarly for other types.
                type_replacements(inner_type, error_exprs)
                    .map(|rep| {
                        quote! { #container_type::new(#rep) }
                    })
                    .collect_vec()
            } else if let Some((collection_type, inner_type)) = known_collection(path) {
                iter::once(quote! { #collection_type::new() })
                    .chain(type_replacements(inner_type, error_exprs).map(|rep| {
                        quote! { #collection_type::from_iter([#rep]) }
                    }))
                    .collect_vec()
            } else if let Some((collection_type, key_type, value_type)) = known_map(path) {
                let key_reps = type_replacements(key_type, error_exprs).collect_vec();
                let val_reps = type_replacements(value_type, error_exprs).collect_vec();
                iter::once(quote! { #collection_type::new() })
                    .chain(
                        key_reps
                            .iter()
                            .cartesian_product(val_reps)
                            .map(|(k, v)| quote! { #collection_type::from_iter([(#k, #v)]) }),
                    )
                    .collect_vec()
            } else if let Some((collection_type, inner_type)) = maybe_collection_or_container(path)
            {
                // Something like `T<A>` or `T<'a, A>`, when we don't know exactly how
                // to call it, but we strongly suspect that you could construct it from
                // an `A`.
                iter::once(quote! { #collection_type::new() })
                    .chain(type_replacements(inner_type, error_exprs).flat_map(|rep| {
                        [
                            quote! { #collection_type::from_iter([#rep]) },
                            quote! { #collection_type::new(#rep) },
                            quote! { #collection_type::from(#rep) },
                        ]
                    }))
                    .collect_vec()
            } else {
                trace!(?type_, "Return type is not recognized, trying Default");
                vec![quote! { Default::default() }]
            }
        }
        Type::Array(TypeArray { elem, len, .. }) =>
        // Generate arrays that repeat each replacement value however many times.
        // In principle we could generate combinations, but that might get very
        // large, and values like "all zeros" and "all ones" seem likely to catch
        // lots of things.
        {
            type_replacements(elem, error_exprs)
                .map(|r| quote! { [ #r; #len ] })
                .collect_vec()
        }
        Type::Slice(TypeSlice { elem, .. }) => iter::once(quote! { Vec::leak(Vec::new()) })
            .chain(type_replacements(elem, error_exprs).map(|r| quote! { Vec::leak(vec![ #r ]) }))
            .collect_vec(),
        Type::Reference(syn::TypeReference {
            mutability: None,
            elem,
            ..
        }) => match &**elem {
            // You can't currently match box patterns in Rust
            Type::Path(path) if path.path.is_ident("str") => {
                vec![quote! { "" }, quote! { "xyzzy" }]
            }
            Type::Slice(TypeSlice { elem, .. }) => iter::once(quote! { Vec::leak(Vec::new()) })
                .chain(
                    type_replacements(elem, error_exprs).map(|r| quote! { Vec::leak(vec![ #r ]) }),
                )
                .collect_vec(),
            _ => type_replacements(elem, error_exprs)
                .map(|rep| {
                    quote! { &#rep }
                })
                .collect_vec(),
        },
        Type::Reference(syn::TypeReference {
            mutability: Some(_),
            elem,
            ..
        }) => match &**elem {
            Type::Slice(TypeSlice { elem, .. }) => iter::once(quote! { Vec::leak(Vec::new()) })
                .chain(
                    type_replacements(elem, error_exprs).map(|r| quote! { Vec::leak(vec![ #r ]) }),
                )
                .collect_vec(),
            _ => {
                // Make &mut with static lifetime by leaking them on the heap.
                type_replacements(elem, error_exprs)
                    .map(|rep| {
                        quote! { Box::leak(Box::new(#rep)) }
                    })
                    .collect_vec()
            }
        },
        Type::Tuple(TypeTuple { elems, .. }) if elems.is_empty() => {
            vec![quote! { () }]
        }
        Type::Tuple(TypeTuple { elems, .. }) => {
            // Generate the cartesian product of replacements of every type within the tuple.
            elems
                .iter()
                .map(|elem| type_replacements(elem, error_exprs).collect_vec())
                .multi_cartesian_product()
                .map(|reps| {
                    quote! { ( #( #reps ),* ) }
                })
                .collect_vec()
        }
        // -> impl Iterator<Item = T>
        Type::ImplTrait(impl_trait) => {
            if let Some(item_type) = match_impl_iterator(impl_trait) {
                iter::once(quote! { ::std::iter::empty() })
                    .chain(
                        type_replacements(item_type, error_exprs)
                            .map(|r| quote! { ::std::iter::once(#r) }),
                    )
                    .collect_vec()
            } else {
                // TODO: Can we do anything with other impl traits?
                vec![]
            }
        }
        Type::Never(_) => {
            vec![]
        }
        _ => {
            trace!(?type_, "Return type is not recognized, trying Default");
            vec![quote! { Default::default() }]
        }
    }
    .into_iter()
}

fn path_ends_with(path: &Path, ident: &str) -> bool {
    path.segments.last().map_or(false, |s| s.ident == ident)
}

fn match_impl_iterator(TypeImplTrait { bounds, .. }: &TypeImplTrait) -> Option<&Type> {
    for bound in bounds {
        if let TypeParamBound::Trait(TraitBound { path, .. }) = bound {
            if path.segments.len() == 1 && path.segments[0].ident == "Iterator" {
                if let PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                    args, ..
                }) = &path.segments[0].arguments
                {
                    if let Some(GenericArgument::AssocType(AssocType { ident, ty, .. })) =
                        args.first()
                    {
                        if ident == "Item" {
                            return Some(ty);
                        }
                    }
                }
            }
        }
    }
    None
}

/// If the type has a single type argument then, perhaps it's a simple container
/// like Box, Cell, Mutex, etc, that can be constructed with `T::new(inner_val)`.
///
/// If so, return the short name (like "Box") and the inner type.
fn known_container(path: &Path) -> Option<(&Ident, &Type)> {
    let last = path.segments.last()?;
    if ["Box", "Cell", "RefCell", "Arc", "Rc", "Mutex"]
        .iter()
        .any(|v| last.ident == v)
    {
        if let PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }) =
            &last.arguments
        {
            // TODO: Skip lifetime args.
            // TODO: Return the path with args stripped out.
            if args.len() == 1 {
                if let Some(GenericArgument::Type(inner_type)) = args.first() {
                    return Some((&last.ident, inner_type));
                }
            }
        }
    }
    None
}

/// Match known simple collections that can be empty or constructed from an
/// iterator.
fn known_collection(path: &Path) -> Option<(&Ident, &Type)> {
    let last = path.segments.last()?;
    if ![
        "BinaryHeap",
        "BTreeSet",
        "HashSet",
        "LinkedList",
        "VecDeque",
    ]
    .iter()
    .any(|v| last.ident == v)
    {
        return None;
    }
    if let PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }) =
        &last.arguments
    {
        // TODO: Skip lifetime args.
        // TODO: Return the path with args stripped out.
        if args.len() == 1 {
            if let Some(GenericArgument::Type(inner_type)) = args.first() {
                return Some((&last.ident, inner_type));
            }
        }
    }
    None
}

/// Match known key-value maps that can be empty or constructed from pair of
/// recursively-generated values.
fn known_map(path: &Path) -> Option<(&Ident, &Type, &Type)> {
    let last = path.segments.last()?;
    if !["BTreeMap", "HashMap"].iter().any(|v| last.ident == v) {
        return None;
    }
    if let PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }) =
        &last.arguments
    {
        // TODO: Skip lifetime args.
        // TODO: Return the path with args stripped out.
        if let Some((GenericArgument::Type(key_type), GenericArgument::Type(value_type))) =
            args.iter().collect_tuple()
        {
            return Some((&last.ident, &key_type, &value_type));
        }
    }
    None
}
/// Match a type with one type argument, which might be a container or collection.
fn maybe_collection_or_container(path: &Path) -> Option<(&Ident, &Type)> {
    let last = path.segments.last()?;
    if let PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }) =
        &last.arguments
    {
        let type_args: Vec<_> = args
            .iter()
            .filter_map(|a| match a {
                GenericArgument::Type(t) => Some(t),
                _ => None,
            })
            .collect();
        // TODO: Return the path with args stripped out.
        if type_args.len() == 1 {
            return Some((&last.ident, type_args.first().unwrap()));
        }
    }
    None
}

fn path_is_float(path: &Path) -> bool {
    ["f32", "f64"].iter().any(|s| path.is_ident(s))
}

fn path_is_unsigned(path: &Path) -> bool {
    ["u8", "u16", "u32", "u64", "u128", "usize"]
        .iter()
        .any(|s| path.is_ident(s))
}

fn path_is_signed(path: &Path) -> bool {
    ["i8", "i16", "i32", "i64", "i128", "isize"]
        .iter()
        .any(|s| path.is_ident(s))
}

fn path_is_nonzero_signed(path: &Path) -> bool {
    if let Some(l) = path.segments.last().map(|p| p.ident.to_string()) {
        matches!(
            l.as_str(),
            "NonZeroIsize"
                | "NonZeroI8"
                | "NonZeroI16"
                | "NonZeroI32"
                | "NonZeroI64"
                | "NonZeroI128",
        )
    } else {
        false
    }
}

fn path_is_nonzero_unsigned(path: &Path) -> bool {
    if let Some(l) = path.segments.last().map(|p| p.ident.to_string()) {
        matches!(
            l.as_str(),
            "NonZeroUsize"
                | "NonZeroU8"
                | "NonZeroU16"
                | "NonZeroU32"
                | "NonZeroU64"
                | "NonZeroU128",
        )
    } else {
        false
    }
}

/// If this is a path ending in `expected_ident`, return the first type argument, ignoring
/// lifetimes.
fn match_first_type_arg<'p>(path: &'p Path, expected_ident: &str) -> Option<&'p Type> {
    // TODO: Maybe match only things with one arg?
    let last = path.segments.last()?;
    if last.ident == expected_ident {
        if let PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }) =
            &last.arguments
        {
            for arg in args {
                match arg {
                    GenericArgument::Type(arg_type) => return Some(arg_type),
                    GenericArgument::Lifetime(_) => (),
                    _ => return None,
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod test {
    use itertools::Itertools;
    use pretty_assertions::assert_eq;
    use syn::{parse_quote, Expr, ReturnType};

    use crate::pretty::ToPrettyString;

    use super::{known_map, return_type_replacements};

    #[test]
    fn recurse_into_result_bool() {
        check_replacements(
            parse_quote! {-> std::result::Result<bool> },
            &[],
            &["Ok(true)", "Ok(false)"],
        );
    }

    #[test]
    fn recurse_into_result_result_bool_with_error_values() {
        check_replacements(
            parse_quote! {-> std::result::Result<Result<bool>> },
            &[parse_quote! { anyhow!("mutated") }],
            &[
                "Ok(Ok(true))",
                "Ok(Ok(false))",
                r#"Ok(Err(anyhow!("mutated")))"#,
                r#"Err(anyhow!("mutated"))"#,
            ],
        );
    }

    #[test]
    fn u16_replacements() {
        check_replacements(parse_quote! { -> u16 }, &[], &["0", "1"]);
    }

    #[test]
    fn isize_replacements() {
        check_replacements(parse_quote! { -> isize }, &[], &["0", "1", "-1"]);
    }

    #[test]
    fn nonzero_integer_replacements() {
        check_replacements(
            parse_quote! { -> std::num::NonZeroIsize },
            &[],
            &["1", "-1"],
        );

        check_replacements(parse_quote! { -> std::num::NonZeroUsize }, &[], &["1"]);

        check_replacements(parse_quote! { -> std::num::NonZeroU32 }, &[], &["1"]);
    }

    #[test]
    fn unit_replacement() {
        check_replacements(parse_quote! { -> () }, &[], &["()"]);
    }

    #[test]
    fn result_unit_replacement() {
        check_replacements(parse_quote! { -> Result<(), Error> }, &[], &["Ok(())"]);

        check_replacements(parse_quote! { -> Result<()> }, &[], &["Ok(())"]);
    }

    #[test]
    fn http_response_replacement() {
        check_replacements(
            parse_quote! { -> HttpResponse },
            &[],
            &["HttpResponse::Ok().finish()"],
        );
    }

    #[test]
    fn option_usize_replacement() {
        check_replacements(
            parse_quote! { -> Option<usize> },
            &[],
            &["None", "Some(0)", "Some(1)"],
        );
    }

    #[test]
    fn box_usize_replacement() {
        check_replacements(
            parse_quote! { -> Box<usize> },
            &[],
            &["Box::new(0)", "Box::new(1)"],
        );
    }

    #[test]
    fn box_unrecognized_type_replacement() {
        check_replacements(
            parse_quote! { -> Box<MyObject> },
            &[],
            &["Box::new(Default::default())"],
        );
    }

    #[test]
    fn vec_string_replacement() {
        check_replacements(
            parse_quote! { -> std::vec::Vec<String> },
            &[],
            &["vec![]", "vec![String::new()]", r#"vec!["xyzzy".into()]"#],
        );
    }

    #[test]
    fn float_replacement() {
        check_replacements(parse_quote! { -> f32 }, &[], &["0.0", "1.0", "-1.0"]);
    }

    #[test]
    fn ref_replacement_recurses() {
        check_replacements(parse_quote! { -> &bool }, &[], &["&true", "&false"]);
    }

    #[test]
    fn array_replacement() {
        check_replacements(
            parse_quote! { -> [u8; 256] },
            &[],
            &["[0; 256]", "[1; 256]"],
        );
    }

    #[test]
    fn arc_replacement() {
        // Also checks that it matches the path, even using an atypical path.
        // TODO: Ideally this would be fully qualified like `alloc::sync::Arc::new(String::new())`.
        check_replacements(
            parse_quote! { -> alloc::sync::Arc<String> },
            &[],
            &["Arc::new(String::new())", r#"Arc::new("xyzzy".into())"#],
        );
    }

    #[test]
    fn rc_replacement() {
        // Also checks that it matches the path, even using an atypical path.
        // TODO: Ideally this would be fully qualified like `alloc::sync::Rc::new(String::new())`.
        check_replacements(
            parse_quote! { -> alloc::sync::Rc<String> },
            &[],
            &["Rc::new(String::new())", r#"Rc::new("xyzzy".into())"#],
        );
    }

    #[test]
    fn btreeset_replacement() {
        check_replacements(
            parse_quote! { -> std::collections::BTreeSet<String> },
            &[],
            &[
                "BTreeSet::new()",
                "BTreeSet::from_iter([String::new()])",
                r#"BTreeSet::from_iter(["xyzzy".into()])"#,
            ],
        );
    }

    #[test]
    fn cow_generates_borrowed_and_owned() {
        check_replacements(
            parse_quote! { -> Cow<'static, str> },
            &[],
            &[
                r#"Cow::Borrowed("")"#,
                r#"Cow::Owned("".to_owned())"#,
                r#"Cow::Borrowed("xyzzy")"#,
                r#"Cow::Owned("xyzzy".to_owned())"#,
            ],
        );
    }

    #[test]
    fn unknown_container_replacement() {
        // This looks like something that holds a &str, and maybe can be constructed
        // from a &str, but we don't know anything else about it, so we just guess.
        check_replacements(
            parse_quote! { -> UnknownContainer<'static, str> },
            &[],
            &[
                "UnknownContainer::new()",
                r#"UnknownContainer::from_iter([""])"#,
                r#"UnknownContainer::new("")"#,
                r#"UnknownContainer::from("")"#,
                r#"UnknownContainer::from_iter(["xyzzy"])"#,
                r#"UnknownContainer::new("xyzzy")"#,
                r#"UnknownContainer::from("xyzzy")"#,
            ],
        );
    }

    #[test]
    fn tuple_combinations() {
        check_replacements(
            parse_quote! { -> (bool, usize) },
            &[],
            &["(true, 0)", "(true, 1)", "(false, 0)", "(false, 1)"],
        )
    }

    #[test]
    fn tuple_combination_longer() {
        check_replacements(
            parse_quote! { -> (bool, Option<String>) },
            &[],
            &[
                "(true, None)",
                "(true, Some(String::new()))",
                r#"(true, Some("xyzzy".into()))"#,
                "(false, None)",
                "(false, Some(String::new()))",
                r#"(false, Some("xyzzy".into()))"#,
            ],
        )
    }

    #[test]
    fn iter_replacement() {
        check_replacements(
            parse_quote! { -> impl Iterator<Item = String> },
            &[],
            &[
                "::std::iter::empty()",
                "::std::iter::once(String::new())",
                r#"::std::iter::once("xyzzy".into())"#,
            ],
        );
    }

    #[test]
    fn slice_replacement() {
        check_replacements(
            parse_quote! { -> [u8] },
            &[],
            &[
                "Vec::leak(Vec::new())",
                "Vec::leak(vec![0])",
                "Vec::leak(vec![1])",
            ],
        );
    }

    #[test]
    fn btreemap_replacement() {
        check_replacements(
            parse_quote! { -> BTreeMap<String, bool> },
            &[],
            &[
                "BTreeMap::new()",
                "BTreeMap::from_iter([(String::new(), true)])",
                "BTreeMap::from_iter([(String::new(), false)])",
                "BTreeMap::from_iter([(\"xyzzy\".into(), true)])",
                "BTreeMap::from_iter([(\"xyzzy\".into(), false)])",
            ],
        );
    }

    fn check_replacements(return_type: ReturnType, error_exprs: &[Expr], expected: &[&str]) {
        assert_eq!(
            return_type_replacements(&return_type, error_exprs)
                .into_iter()
                .map(|t| t.to_pretty_string())
                .collect_vec(),
            expected
        );
    }

    #[test]
    fn match_map() {
        assert!(known_map(&parse_quote! { BTreeMap<String, usize> }).is_some());
        assert!(known_map(&parse_quote! { HashMap<(usize, usize), bool> }).is_some());
        assert!(known_map(&parse_quote! { Option<(usize, usize)> }).is_none());
    }
}

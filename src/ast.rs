//! Utilities for syn ASTs.

use syn::Attribute;
use tracing::debug;

/// True if any of the attrs indicate that we should skip this node and everything inside it.
///
/// This checks for `#[cfg(test)]`, `#[test]`, and `#[mutants::skip]`.
pub fn attrs_excluded(attrs: &[Attribute]) -> bool {
    attrs
        .iter()
        .any(|attr| attr_is_cfg_test(attr) || attr_is_test(attr) || attr_is_mutants_skip(attr))
}

/// True if the attribute looks like `#[cfg(test)]`, or has "test"
/// anywhere in it.
pub fn attr_is_cfg_test(attr: &Attribute) -> bool {
    if !path_is(attr.path(), &["cfg"]) {
        return false;
    }
    let mut contains_test = false;
    if let Err(err) = attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("test") {
            contains_test = true;
        }
        Ok(())
    }) {
        debug!(
            ?err,
            ?attr,
            "Attribute is not in conventional form; skipped"
        );
        return false;
    }
    contains_test
}

/// True if the attribute is `#[test]`.
pub fn attr_is_test(attr: &Attribute) -> bool {
    attr.path().is_ident("test")
}

pub fn path_is(path: &syn::Path, idents: &[&str]) -> bool {
    path.segments.iter().map(|ps| &ps.ident).eq(idents.iter())
}

/// True if the attribute contains `mutants::skip`.
///
/// This for example returns true for `#[mutants::skip] or `#[cfg_attr(test, mutants::skip)]`.
pub fn attr_is_mutants_skip(attr: &Attribute) -> bool {
    if path_is(attr.path(), &["mutants", "skip"]) {
        return true;
    }
    if !path_is(attr.path(), &["cfg_attr"]) {
        return false;
    }
    let mut skip = false;
    if let Err(err) = attr.parse_nested_meta(|meta| {
        if path_is(&meta.path, &["mutants", "skip"]) {
            skip = true
        }
        Ok(())
    }) {
        debug!(
            ?attr,
            ?err,
            "Attribute is not a path with attributes; skipping"
        );
        return false;
    }
    skip
}

/// True if the block (e.g. the contents of a function) is empty.
pub fn block_is_empty(block: &syn::Block) -> bool {
    block.stmts.is_empty()
}

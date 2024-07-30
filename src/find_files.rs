// Copyright 2021-2024 Martin Pool

//! Walk a source tree finding all the mods and loading source files.
//!
//! This is only interested in `mod` statements, not `use` or `extern crate`,
//! and it doesn't generate mutants.
//!
//! Walking the tree starts with some root files known to the build tool:
//! e.g. for cargo they are identified from the targets. The tree walker then
//! follows `mod` statements to recursively visit other referenced files.

use std::collections::VecDeque;
use std::vec;

use syn::ext::IdentExt;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Attribute, File};
use tracing::{debug, debug_span, error, trace, trace_span, warn};

use crate::source::SourceFile;
use crate::span::Span;
use crate::*;

/// Discover all mutants and all source files.
///
/// The list of source files includes even those with no mutants.
///
pub fn find_source_files(
    workspace_dir: &Utf8Path,
    top_source_files: &[SourceFile],
    options: &Options,
    console: &Console,
) -> Result<Vec<SourceFile>> {
    // console.start_find_files(); // TODO
    let mut file_queue: VecDeque<SourceFile> = top_source_files.iter().cloned().collect();
    let mut files: Vec<SourceFile> = Vec::new();
    while let Some(source_file) = file_queue.pop_front() {
        // console.find_files_update(files.len(), &source_file.tree_relative_slashes()); // TODO
        check_interrupted()?;
        let external_mods = walk_file(&source_file)?;
        // We'll still walk down through files that don't match globs, so that
        // we have a chance to find modules underneath them. However, we won't
        // collect any mutants from them, and they don't count as "seen" for
        // `--list-files`.
        for mod_namespace in &external_mods {
            if let Some(mod_path) = find_mod_source(workspace_dir, &source_file, mod_namespace)? {
                file_queue.extend(SourceFile::new(
                    workspace_dir,
                    mod_path,
                    &source_file.package,
                    false,
                )?)
            }
        }
        let path = &source_file.tree_relative_path;
        if let Some(examine_globset) = &options.examine_globset {
            if !examine_globset.is_match(path) {
                trace!("{path:?} does not match examine globset");
                continue;
            }
        }
        if let Some(exclude_globset) = &options.exclude_globset {
            if exclude_globset.is_match(path) {
                trace!("{path:?} excluded by globset");
                continue;
            }
        }
        files.push(source_file);
    }
    // console.end_find_files(); // TODO
    Ok(files)
}

/// Find all modules referenced by a `mod` statement in a source file, which
/// will need to later be visited.
fn walk_file(source_file: &SourceFile) -> Result<Vec<Vec<ModNamespace>>> {
    let _span = debug_span!("source_file", path = source_file.tree_relative_slashes()).entered();
    debug!("visit source file");
    let syn_file = syn::parse_str::<syn::File>(source_file.code())
        .with_context(|| format!("failed to parse {}", source_file.tree_relative_slashes()))?;
    let mut visitor = Visitor {
        external_mods: Vec::new(),
        mod_namespace_stack: Vec::new(),
        source_file: source_file.clone(),
    };
    visitor.visit_file(&syn_file);
    Ok(visitor.external_mods)
}

/// Namespace for a module defined in a `mod foo { ... }` block or `mod foo;` statement
///
/// In the context of resolving modules, a module "path" (and to some extent "name") is ambiguous:
/// paths may describe a sequence of identifiers in code (e.g. `crate::foo::bar`) or sequence of
/// folder and file names on the filesystem (e.g. `src/foo/bar.rs`).
///
/// The field and method names in this struct distinguish between the uses of path elements.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ModNamespace {
    /// Identifier of the module (e.g. `foo` for `mod foo;`)
    name: String,
    /// File location override for the module, if specified by `#[path="file"]`
    ///
    /// Note that `mod foo { ... }` blocks can also have a file location specified,
    /// which affects the filesystem location of all child `mod bar;` statements.
    path_attribute: Option<Utf8PathBuf>,
    /// Location of the module definition in the source file
    source_location: Span,
}

impl ModNamespace {
    /// Returns the name of the module for filesystem purposes
    fn get_filesystem_name(&self) -> &Utf8Path {
        self.path_attribute
            .as_ref()
            .map(Utf8PathBuf::as_path)
            .unwrap_or(Utf8Path::new(&self.name))
    }
}

/// `syn` visitor that recursively traverses a source file, collecting references
/// to other files that should be visited.
struct Visitor {
    /// The file being visited.
    source_file: SourceFile,

    /// The stack of modules namespaces that we're currently inside, from
    /// visiting `mod foo { ... }` statements.
    ///
    /// This is a subsequence of `namespace_stack` (with `#[path="..."]` information),
    /// containing only elements that form a module path.
    mod_namespace_stack: Vec<ModNamespace>,

    /// The names from `mod foo;` statements that should be visited later,
    /// namespaced relative to the source file
    external_mods: Vec<Vec<ModNamespace>>,
}

impl<'ast> Visit<'ast> for Visitor {
    /// Visit a source file.
    fn visit_file(&mut self, i: &'ast File) {
        // No trace here; it's created per file for the whole visitor
        if attrs_excluded(&i.attrs) {
            trace!("file excluded by attrs");
            return;
        }
        syn::visit::visit_file(self, i);
    }

    /// Visit `mod foo { ... }` or `mod foo;`.
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let mod_name = node.ident.unraw().to_string();
        let _span = trace_span!("mod", line = node.mod_token.span.start().line, mod_name).entered();
        if attrs_excluded(&node.attrs) {
            trace!("mod excluded by attrs");
            return;
        }

        let source_location = Span::from(node.span());

        // Extract path attribute value, if any (e.g. `#[path="..."]`)
        let path_attribute = match find_path_attribute(&node.attrs) {
            Ok(path) => path,
            Err(path_attribute) => {
                let definition_site = self
                    .source_file
                    .format_source_location(source_location.start);
                error!(?path_attribute, ?definition_site, %mod_name, "invalid filesystem traversal in mod path attribute");
                return;
            }
        };
        let mod_namespace = ModNamespace {
            name: mod_name,
            path_attribute,
            source_location,
        };
        self.mod_namespace_stack.push(mod_namespace.clone());

        // If there's no content in braces, then this is a `mod foo;`
        // statement referring to an external file. We remember the module
        // name and then later look for the file.
        if node.content.is_none() {
            // If we're already inside `mod a { ... }` and see `mod b;` then
            // remember [a, b] as an external module to visit later.
            self.external_mods.push(self.mod_namespace_stack.clone());
        }
        syn::visit::visit_item_mod(self, node);
        assert_eq!(self.mod_namespace_stack.pop(), Some(mod_namespace));
    }
}

/// Find a new source file referenced by a `mod` statement.
///
/// Possibly, our heuristics just won't be able to find which file it is,
/// in which case we return `Ok(None)`.
fn find_mod_source(
    tree_root: &Utf8Path,
    parent: &SourceFile,
    mod_namespace: &[ModNamespace],
) -> Result<Option<Utf8PathBuf>> {
    // First, work out whether the mod will be a sibling in the same directory, or
    // in a child directory.
    //
    // 1. The parent is "src/foo.rs" and `mod bar` means "src/foo/bar.rs".
    //
    // 2. The parent is "src/lib.rs" (a target top file) and `mod bar` means "src/bar.rs".
    //
    // 3. The parent is "src/foo/mod.rs" and so `mod bar` means "src/foo/bar.rs".
    //
    // 4. A path attribute on a mod statement when there is no enclosing mod block
    //     E.g. for parent file "src/a/parent_file.rs",
    //     ```
    //     // `path` is relative to the directory where the source file is located
    //     #[path="foo_file.rs"] // resolves to: src/a/foo_file.rs
    //     mod foo;
    //
    //     mod bar {
    //         // `path` is relative to the directory of the enclosing module block
    //         #[path="baz_file.rs"] // resolves to: src/a/parent_file/bar/baz_file.rs
    //         mod baz;
    //     }
    //     ```
    //
    // Having determined the right directory then we can follow the path attribute, or
    // if no path is specified, then look for either `foo.rs` or `foo/mod.rs`.

    let (mod_child, mod_parents) = mod_namespace.split_last().expect("mod namespace is empty");

    // TODO: Beyond #115, we should probably remove all special handling of
    // `mod.rs` here by remembering how we found this file, and whether it
    // is above or inside the directory corresponding to its module?

    let parent_path = &parent.tree_relative_path;
    let mut search_dir = if parent.is_top
        || parent_path.ends_with("mod.rs")
        // NOTE: Path attribute on a top-level `mod foo;` (no enclosing block)
        //       ignores the parent module path
        || (mod_child.path_attribute.is_some() && mod_parents.is_empty())
    {
        parent_path
            .parent()
            .expect("mod path has no parent")
            .to_owned() // src/lib.rs -> src/
    } else {
        parent_path.with_extension("") // foo.rs -> foo/
    };

    search_dir.extend(mod_parents.iter().map(ModNamespace::get_filesystem_name));

    let mod_child_candidates = if let Some(filesystem_name) = &mod_child.path_attribute {
        vec![search_dir.join(filesystem_name)]
    } else {
        [".rs", "/mod.rs"]
            .iter()
            .map(|tail| search_dir.join(mod_child.name.clone() + tail))
            .collect()
    };

    let mut tried_paths = Vec::new();
    for relative_path in mod_child_candidates {
        let full_path = tree_root.join(&relative_path);
        if full_path.is_file() {
            trace!("found submodule in {full_path}");
            return Ok(Some(relative_path));
        } else {
            tried_paths.push(full_path);
        }
    }
    let mod_name = &mod_child.name;
    let definition_site = parent.format_source_location(mod_child.source_location.start);
    warn!(?definition_site, %mod_name, ?tried_paths, "referent of mod not found");
    Ok(None)
}

/// True if the signature of a function is such that it should be excluded.
fn fn_sig_excluded(sig: &syn::Signature) -> bool {
    if sig.unsafety.is_some() {
        trace!("Skip unsafe fn");
        true
    } else {
        false
    }
}

/// True if any of the attrs indicate that we should skip this node and everything inside it.
///
/// This checks for `#[cfg(test)]`, `#[test]`, and `#[mutants::skip]`.
fn attrs_excluded(attrs: &[Attribute]) -> bool {
    attrs
        .iter()
        .any(|attr| attr_is_cfg_test(attr) || attr_is_test(attr) || attr_is_mutants_skip(attr))
}

/// True if the block (e.g. the contents of a function) is empty.
fn block_is_empty(block: &syn::Block) -> bool {
    block.stmts.is_empty()
}

/// True if the attribute looks like `#[cfg(test)]`, or has "test"
/// anywhere in it.
fn attr_is_cfg_test(attr: &Attribute) -> bool {
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
fn attr_is_test(attr: &Attribute) -> bool {
    attr.path().is_ident("test")
}

fn path_is(path: &syn::Path, idents: &[&str]) -> bool {
    path.segments.iter().map(|ps| &ps.ident).eq(idents.iter())
}

/// True if the attribute contains `mutants::skip`.
///
/// This for example returns true for `#[mutants::skip] or `#[cfg_attr(test, mutants::skip)]`.
fn attr_is_mutants_skip(attr: &Attribute) -> bool {
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

/// Finds the first path attribute (`#[path = "..."]`)
///
/// # Errors
/// Returns an error if the path attribute contains a dubious path (leading `/`)
fn find_path_attribute(attrs: &[Attribute]) -> std::result::Result<Option<Utf8PathBuf>, String> {
    attrs
        .iter()
        .find_map(|attr| match &attr.meta {
            syn::Meta::NameValue(meta) if meta.path.is_ident("path") => {
                let syn::Expr::Lit(expr_lit) = &meta.value else {
                    return None;
                };
                let syn::Lit::Str(lit_str) = &expr_lit.lit else {
                    return None;
                };
                let path = lit_str.value();

                // refuse to follow absolute paths
                if path.starts_with('/') {
                    Some(Err(path))
                } else {
                    Some(Ok(Utf8PathBuf::from(path)))
                }
            }
            _ => None,
        })
        .transpose()
}

#[cfg(test)]
mod test {
    use indoc::indoc;
    use itertools::Itertools;
    use proc_macro2::TokenStream;
    use quote::quote;

    use super::*;
    use crate::package::Package;

    /// We don't visit functions inside files marked with `#![cfg(test)]`.
    #[test]
    fn no_mutants_in_files_with_inner_cfg_test_attribute() {
        let options = Options::default();
        let console = Console::new();
        let workspace = Workspace::open("testdata/cfg_test_inner").unwrap();
        let discovered = workspace
            .discover(&PackageFilter::All, &options, &console)
            .unwrap();
        assert_eq!(discovered.mutants.as_slice(), &[]);
    }

    /// Helper function for `find_path_attribute` tests
    fn run_find_path_attribute(
        token_stream: TokenStream,
    ) -> std::result::Result<Option<Utf8PathBuf>, String> {
        let token_string = token_stream.to_string();
        let item_mod = syn::parse_str::<syn::ItemMod>(&token_string).unwrap_or_else(|err| {
            panic!("Failed to parse test case token stream: {token_string}\n{err}")
        });
        find_path_attribute(&item_mod.attrs)
    }

    #[test]
    fn find_path_attribute_on_module_item() {
        let outer = run_find_path_attribute(quote! {
            #[path = "foo_file.rs"]
            mod foo;
        });
        assert_eq!(outer, Ok(Some(Utf8PathBuf::from("foo_file.rs"))));

        let inner = run_find_path_attribute(quote! {
            mod foo {
                #![path = "foo_folder"]

                #[path = "file_for_bar.rs"]
                mod bar;
            }
        });
        assert_eq!(inner, Ok(Some(Utf8PathBuf::from("foo_folder"))));
    }

    #[test]
    fn reject_module_path_absolute() {
        // dots are valid
        let dots = run_find_path_attribute(quote! {
            #[path = "contains/../dots.rs"]
            mod dots;
        });
        assert_eq!(dots, Ok(Some(Utf8PathBuf::from("contains/../dots.rs"))));

        let dots_inner = run_find_path_attribute(quote! {
            mod dots_in_path {
                #![path = "contains/../dots"]
            }
        });
        assert_eq!(dots_inner, Ok(Some(Utf8PathBuf::from("contains/../dots"))));

        let leading_slash = run_find_path_attribute(quote! {
            #[path = "/leading_slash.rs"]
            mod dots;
        });
        assert_eq!(leading_slash, Err("/leading_slash.rs".to_owned()));

        let allow_other_slashes = run_find_path_attribute(quote! {
            #[path = "foo/other/slashes/are/allowed.rs"]
            mod dots;
        });
        assert_eq!(
            allow_other_slashes,
            Ok(Some(Utf8PathBuf::from("foo/other/slashes/are/allowed.rs")))
        );

        let leading_slash2 = run_find_path_attribute(quote! {
            #[path = "/leading_slash/../and_dots.rs"]
            mod dots;
        });
        assert_eq!(
            leading_slash2,
            Err("/leading_slash/../and_dots.rs".to_owned())
        );
    }
}

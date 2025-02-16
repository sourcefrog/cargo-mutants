// Copyright 2021 - 2025 Martin Pool

//! Visit all the files in a source tree, and then the AST of each file,
//! to discover mutation opportunities.
//!
//! Walking the tree starts with some root files known to the build tool:
//! e.g. for cargo they are identified from the targets. The tree walker then
//! follows `mod` statements to recursively visit other referenced files.

#![warn(clippy::pedantic)]

use std::collections::VecDeque;
use std::sync::Arc;
use std::vec;

use camino::{Utf8Path, Utf8PathBuf};
use proc_macro2::{Ident, TokenStream};
use quote::{quote, ToTokens};
use syn::ext::IdentExt;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Attribute, BinOp, Block, Expr, ExprPath, File, ItemFn, ReturnType, Signature, UnOp};
use tracing::{debug, debug_span, error, info, trace, trace_span, warn};

use crate::console::WalkProgress;
use crate::fnvalue::return_type_replacements;
use crate::mutant::Function;
use crate::package::Package;
use crate::pretty::ToPrettyString;
use crate::source::SourceFile;
use crate::span::Span;
use crate::{check_interrupted, Console, Context, Genre, Mutant, Options, Result};

/// Mutants and files discovered in a source tree.
///
/// Files are listed separately so that we can represent files that
/// were visited but that produced no mutants.
pub struct Discovered {
    pub mutants: Vec<Mutant>,
    pub files: Vec<SourceFile>,
}

impl Discovered {
    pub(crate) fn remove_previously_caught(&mut self, previously_caught: &[String]) {
        self.mutants.retain(|m| {
            let name = m.name(true);
            let c = previously_caught.contains(&name);
            if c {
                trace!(?name, "skip previously caught mutant");
            }
            !c
        });
    }
}

/// Discover all mutants and all source files.
///
/// The returned `Discovered` struct contains all mutants found in the
/// source tree, and also a list of all source files visited (whether
/// they generated mutants or not).
pub fn walk_tree(
    workspace_dir: &Utf8Path,
    packages: &[Arc<Package>],
    options: &Options,
    console: &Console,
) -> Result<Discovered> {
    let mut mutants = Vec::new();
    let mut files = Vec::new();
    let error_exprs = options.parsed_error_exprs()?;
    let progress = console.start_walk_tree();
    for package in packages {
        let (mut package_mutants, mut package_files) =
            walk_package(workspace_dir, package, &error_exprs, &progress, options)?;
        mutants.append(&mut package_mutants);
        files.append(&mut package_files);
    }
    progress.finish();
    Ok(Discovered { mutants, files })
}

/// Walk one package, starting from its top files, discovering files
/// and mutants.
#[allow(clippy::from_iter_instead_of_collect)]
fn walk_package(
    workspace_dir: &Utf8Path,
    package: &Package,
    error_exprs: &[Expr],
    progress: &WalkProgress,
    options: &Options,
) -> Result<(Vec<Mutant>, Vec<SourceFile>)> {
    let mut mutants = Vec::new();
    let mut files = Vec::new();
    let mut filename_queue =
        VecDeque::from_iter(package.top_sources.iter().map(|p| (p.to_owned(), true)));
    while let Some((path, package_top)) = filename_queue.pop_front() {
        let Some(source_file) = SourceFile::load(workspace_dir, &path, package, package_top)?
        else {
            info!("Skipping source file outside of tree: {path:?}");
            continue;
        };
        progress.increment_files(1);
        check_interrupted()?;
        let (mut file_mutants, external_mods) = walk_file(&source_file, error_exprs, options)?;
        file_mutants.retain(|m| options.allows_mutant(m));
        progress.increment_mutants(file_mutants.len());
        // TODO: It would be better not to spend time generating mutants from
        // files that are not going to be visited later. However, we probably do
        // still want to walk them to find modules that are referenced by them.
        // since otherwise it could be pretty confusing that lower files are not
        // visited.
        //
        // We'll still walk down through files that don't match globs, so that
        // we have a chance to find modules underneath them. However, we won't
        // collect any mutants from them, and they don't count as "seen" for
        // `--list-files`.
        for mod_namespace in &external_mods {
            if let Some(mod_path) = find_mod_source(workspace_dir, &source_file, mod_namespace) {
                filename_queue.push_back((mod_path, false));
            }
        }
        if !options.allows_source_file_path(&source_file.tree_relative_path) {
            continue;
        }
        mutants.append(&mut file_mutants);
        files.push(source_file);
    }
    Ok((mutants, files))
}

/// Find all possible mutants in a source file.
///
/// Returns the mutants found, and the names of modules referenced by `mod` statements
/// that should be visited later.
fn walk_file(
    source_file: &SourceFile,
    error_exprs: &[Expr],
    options: &Options,
) -> Result<(Vec<Mutant>, Vec<ExternalModRef>)> {
    let _span = debug_span!("source_file", path = source_file.tree_relative_slashes()).entered();
    debug!("visit source file");
    let syn_file = syn::parse_str::<syn::File>(source_file.code())
        .with_context(|| format!("failed to parse {}", source_file.tree_relative_slashes()))?;
    let mut visitor = DiscoveryVisitor {
        error_exprs,
        external_mods: Vec::new(),
        mutants: Vec::new(),
        mod_namespace_stack: Vec::new(),
        namespace_stack: Vec::new(),
        fn_stack: Vec::new(),
        source_file: source_file.clone(),
        options,
    };
    visitor.visit_file(&syn_file);
    Ok((visitor.mutants, visitor.external_mods))
}

/// For testing: parse and generate mutants from one single file provided as a string.
///
/// The source code is assumed to be named `src/main.rs` with a fixed package name.
#[cfg(test)]
pub fn mutate_source_str(code: &str, options: &Options) -> Result<Vec<Mutant>> {
    let source_file = SourceFile::for_tests(
        Utf8Path::new("src/main.rs"),
        code,
        "cargo-mutants-testdata-internal",
        true,
    );
    let (mutants, _) = walk_file(&source_file, &options.parsed_error_exprs()?, options)?;
    Ok(mutants)
}

/// Reference to an external module from a source file.
///
/// This is approximately a list of namespace components like `["foo", "bar"]` for
/// `foo::bar`, but each may also be decorated with a `#[path="..."]` attribute,
/// and they're attributed to a location in the source.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ExternalModRef {
    /// Namespace components of the module path
    parts: Vec<ModNamespace>,
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
            .map_or(Utf8Path::new(&self.name), Utf8PathBuf::as_path)
    }
}

/// `syn` visitor that recursively traverses the syntax tree, accumulating places
/// that could be mutated.
///
/// As it walks the tree, it accumulates within itself a list of mutation opportunities,
/// and other files referenced by `mod` statements that should be visited later.
struct DiscoveryVisitor<'o> {
    /// All the mutants generated by visiting the file.
    mutants: Vec<Mutant>,

    /// The file being visited.
    source_file: SourceFile,

    /// The stack of modules namespaces that we're currently inside, from
    /// visiting `mod foo { ... }` statements.
    ///
    /// This is a subsequence of `namespace_stack` (with `#[path="..."]` information),
    /// containing only elements that form a module path.
    mod_namespace_stack: Vec<ModNamespace>,

    /// The stack of namespaces, loosely defined, that we're inside.
    ///
    /// Basically these are names or strings that can be concatenated with `::`
    /// to form a name that meaningfully describes where we are; it might not
    /// exactly be valid Rust.
    ///
    /// For example, this includes mods, fns, impls, etc.
    namespace_stack: Vec<String>,

    /// The functions we're inside.
    ///
    /// Empty at the top level, often has one element, but potentially more if
    /// there are nested functions.
    fn_stack: Vec<Arc<Function>>,

    /// The names from `mod foo;` statements that should be visited later,
    /// namespaced relative to the source file
    external_mods: Vec<ExternalModRef>,

    /// Parsed error expressions, from the config file or command line.
    error_exprs: &'o [Expr],

    options: &'o Options,
}

impl DiscoveryVisitor<'_> {
    fn enter_function(
        &mut self,
        function_name: &Ident,
        return_type: &ReturnType,
        span: proc_macro2::Span,
    ) -> Arc<Function> {
        self.namespace_stack.push(function_name.to_string());
        let full_function_name = self.namespace_stack.join("::");
        let function = Arc::new(Function {
            function_name: full_function_name,
            return_type: return_type.to_pretty_string(),
            span: span.into(),
        });
        self.fn_stack.push(Arc::clone(&function));
        function
    }

    fn leave_function(&mut self, function: Arc<Function>) {
        self.namespace_stack
            .pop()
            .expect("Namespace stack should not be empty");
        assert_eq!(
            self.fn_stack.pop(),
            Some(function),
            "Function stack mismatch"
        );
    }

    /// Record that we generated some mutants.
    fn collect_mutant(&mut self, span: Span, replacement: &TokenStream, genre: Genre) {
        self.mutants.push(Mutant {
            source_file: self.source_file.clone(),
            function: self.fn_stack.last().cloned(),
            span,
            replacement: replacement.to_pretty_string(),
            genre,
        });
    }

    fn collect_fn_mutants(&mut self, sig: &Signature, block: &Block) {
        if let Some(function) = self.fn_stack.last().cloned() {
            let body_span = function_body_span(block).expect("Empty function body");
            let repls = return_type_replacements(&sig.output, self.error_exprs);
            if repls.is_empty() {
                debug!(
                    function_name = function.function_name,
                    return_type = function.return_type,
                    "No mutants generated for this return type"
                );
            } else {
                let orig_block = block.to_token_stream().to_pretty_string();
                for rep in repls {
                    // Comparing strings is a kludge for proc_macro2 not (yet) apparently
                    // exposing any way to compare token streams...
                    //
                    // TODO: Maybe this should move into collect_mutant, but at the moment
                    // FnValue is the only genre that seems able to generate no-ops.
                    //
                    // The original block has braces and the replacements don't, so put
                    // them back for the comparison...
                    let new_block = quote!( { #rep } ).to_token_stream().to_pretty_string();
                    // dbg!(&orig_block, &new_block);
                    if orig_block == new_block {
                        debug!("Replacement is the same as the function body; skipping");
                    } else {
                        self.collect_mutant(body_span, &rep, Genre::FnValue);
                    }
                }
            }
        } else {
            warn!("collect_fn_mutants called while not in a function?");
        }
    }

    /// Call a function with a namespace pushed onto the stack.
    ///
    /// This is used when recursively descending into a namespace.
    fn in_namespace<F, T>(&mut self, name: &str, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        self.namespace_stack.push(name.to_owned());
        let r = f(self);
        assert_eq!(self.namespace_stack.pop().unwrap(), name);
        r
    }
}

impl<'ast> Visit<'ast> for DiscoveryVisitor<'_> {
    fn visit_expr_call(&mut self, i: &'ast syn::ExprCall) {
        let _span = trace_span!("expr_call", line = i.span().start().line).entered();
        if attrs_excluded(&i.attrs) {
            return;
        }
        if let Expr::Path(ExprPath { path, .. }) = &*i.func {
            debug!(path = path.to_pretty_string(), "visit call");
            if let Some(hit) = self
                .options
                .skip_calls
                .iter()
                .find(|s| path_ends_with(path, s))
            {
                trace!("skip call to {hit}");
                return;
            }
        }
        syn::visit::visit_expr_call(self, i);
    }

    fn visit_expr_method_call(&mut self, i: &'ast syn::ExprMethodCall) {
        let _span = trace_span!("expr_method_call", line = i.span().start().line).entered();
        if attrs_excluded(&i.attrs) {
            return;
        }
        if let Some(hit) = self.options.skip_calls.iter().find(|s| i.method == s) {
            trace!("skip method call to {hit}");
            return;
        }
        syn::visit::visit_expr_method_call(self, i);
    }

    /// Visit a source file.
    fn visit_file(&mut self, i: &'ast File) {
        // No trace here; it's created per file for the whole visitor
        if attrs_excluded(&i.attrs) {
            trace!("file excluded by attrs");
            return;
        }
        syn::visit::visit_file(self, i);
    }

    /// Visit top-level `fn foo()`.
    fn visit_item_fn(&mut self, i: &'ast ItemFn) {
        let function_name = i.sig.ident.to_pretty_string();
        let _span = trace_span!(
            "fn",
            line = i.sig.fn_token.span.start().line,
            name = function_name
        )
        .entered();
        trace!("visit fn");
        if fn_sig_excluded(&i.sig) || attrs_excluded(&i.attrs) || block_is_empty(&i.block) {
            return;
        }
        let function = self.enter_function(&i.sig.ident, &i.sig.output, i.span());
        self.collect_fn_mutants(&i.sig, &i.block);
        syn::visit::visit_item_fn(self, i);
        self.leave_function(function);
    }

    /// Visit `fn foo()` within an `impl`.
    fn visit_impl_item_fn(&mut self, i: &'ast syn::ImplItemFn) {
        // Don't look inside constructors (called "new") because there's often no good
        // alternative.
        let function_name = i.sig.ident.to_pretty_string();
        let _span = trace_span!(
            "fn",
            line = i.sig.fn_token.span.start().line,
            name = function_name
        )
        .entered();
        if fn_sig_excluded(&i.sig)
            || attrs_excluded(&i.attrs)
            || i.sig.ident == "new"
            || block_is_empty(&i.block)
        {
            return;
        }
        let function = self.enter_function(&i.sig.ident, &i.sig.output, i.span());
        self.collect_fn_mutants(&i.sig, &i.block);
        syn::visit::visit_impl_item_fn(self, i);
        self.leave_function(function);
    }

    /// Visit `fn foo() { ... }` within a trait, i.e. a default implementation of a function.
    fn visit_trait_item_fn(&mut self, i: &'ast syn::TraitItemFn) {
        let function_name = i.sig.ident.to_pretty_string();
        let _span = trace_span!(
            "fn",
            line = i.sig.fn_token.span.start().line,
            name = function_name
        )
        .entered();
        if fn_sig_excluded(&i.sig) || attrs_excluded(&i.attrs) || i.sig.ident == "new" {
            return;
        }
        if let Some(block) = &i.default {
            if block_is_empty(block) {
                return;
            }
            let function = self.enter_function(&i.sig.ident, &i.sig.output, i.span());
            self.collect_fn_mutants(&i.sig, block);
            syn::visit::visit_trait_item_fn(self, i);
            self.leave_function(function);
        }
    }

    /// Visit `impl Foo { ...}` or `impl Debug for Foo { ... }`.
    fn visit_item_impl(&mut self, i: &'ast syn::ItemImpl) {
        if attrs_excluded(&i.attrs) {
            return;
        }
        let type_name = i.self_ty.to_pretty_string();
        let name = if let Some((_, trait_path, _)) = &i.trait_ {
            if path_ends_with(trait_path, "Default") {
                // Can't think of how to generate a viable different default.
                return;
            }
            format!("<impl {trait} for {type_name}>", trait = trait_path.to_pretty_string())
        } else {
            type_name
        };
        self.in_namespace(&name, |v| syn::visit::visit_item_impl(v, i));
    }

    /// Visit `trait Foo { ... }`
    fn visit_item_trait(&mut self, i: &'ast syn::ItemTrait) {
        let name = i.ident.to_pretty_string();
        let _span = trace_span!("trait", line = i.span().start().line, name).entered();
        if attrs_excluded(&i.attrs) {
            return;
        }
        self.in_namespace(&name, |v| syn::visit::visit_item_trait(v, i));
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
            self.external_mods.push(ExternalModRef {
                parts: self.mod_namespace_stack.clone(),
            });
        }
        self.in_namespace(&mod_namespace.name, |v| syn::visit::visit_item_mod(v, node));
        assert_eq!(self.mod_namespace_stack.pop(), Some(mod_namespace));
    }

    /// Visit `a op b` expressions.
    fn visit_expr_binary(&mut self, i: &'ast syn::ExprBinary) {
        let _span = trace_span!("binary", line = i.op.span().start().line).entered();
        trace!("visit binary operator");
        if attrs_excluded(&i.attrs) {
            return;
        }
        let replacements = match i.op {
            // We don't generate `<=` from `==` because it can too easily go
            // wrong with unsigned types compared to 0.

            // We try replacing logical ops with == and !=, which are effectively
            // XNOR and XOR when applied to booleans. However, they're often unviable
            // because they require parenthesis for disambiguation in many expressions.
            BinOp::Eq(_) => vec![quote! { != }],
            BinOp::Ne(_) => vec![quote! { == }],
            BinOp::And(_) => vec![quote! { || }],
            BinOp::Or(_) => vec![quote! { && }],
            BinOp::Lt(_) => vec![quote! { == }, quote! {>}, quote! { <= }],
            BinOp::Gt(_) => vec![quote! { == }, quote! {<}, quote! { => }],
            BinOp::Le(_) => vec![quote! {>}],
            BinOp::Ge(_) => vec![quote! {<}],
            BinOp::Add(_) => vec![quote! {-}, quote! {*}],
            BinOp::AddAssign(_) => vec![quote! {-=}, quote! {*=}],
            BinOp::Sub(_) | BinOp::Mul(_) => vec![quote! {+}, quote! {/}],
            BinOp::SubAssign(_) | BinOp::MulAssign(_) => vec![quote! {+=}, quote! {/=}],
            BinOp::Div(_) => vec![quote! {%}, quote! {*}],
            BinOp::DivAssign(_) => vec![quote! {%=}, quote! {*=}],
            BinOp::Rem(_) => vec![quote! {/}, quote! {+}],
            BinOp::RemAssign(_) => vec![quote! {/=}, quote! {+=}],
            BinOp::Shl(_) => vec![quote! {>>}],
            BinOp::ShlAssign(_) => vec![quote! {>>=}],
            BinOp::Shr(_) => vec![quote! {<<}],
            BinOp::ShrAssign(_) => vec![quote! {<<=}],
            BinOp::BitAnd(_) => vec![quote! {|}, quote! {^}],
            BinOp::BitAndAssign(_) => vec![quote! {|=}, quote! {^=}],
            BinOp::BitOr(_) => vec![quote! {&}, quote! {^}],
            BinOp::BitOrAssign(_) => vec![quote! {&=}, quote! {^=}],
            BinOp::BitXor(_) => vec![quote! {|}, quote! {&}],
            BinOp::BitXorAssign(_) => vec![quote! {|=}, quote! {&=}],
            _ => {
                trace!(
                    op = i.op.to_pretty_string(),
                    "No mutants generated for this binary operator"
                );
                Vec::new()
            }
        };
        replacements
            .into_iter()
            .for_each(|rep| self.collect_mutant(i.op.span().into(), &rep, Genre::BinaryOperator));
        syn::visit::visit_expr_binary(self, i);
    }

    fn visit_expr_unary(&mut self, i: &'ast syn::ExprUnary) {
        let _span = trace_span!("unary", line = i.op.span().start().line).entered();
        trace!("visit unary operator");
        if attrs_excluded(&i.attrs) {
            return;
        }
        match i.op {
            UnOp::Not(_) | UnOp::Neg(_) => {
                self.collect_mutant(i.op.span().into(), &quote! {}, Genre::UnaryOperator);
            }
            _ => {
                trace!(
                    op = i.op.to_pretty_string(),
                    "No mutants generated for this unary operator"
                );
            }
        };
        syn::visit::visit_expr_unary(self, i);
    }

    fn visit_expr_match(&mut self, i: &'ast syn::ExprMatch) {
        let _span = trace_span!("match", line = i.span().start().line).entered();

        // While it's not currently possible to annotate expressions with custom attributes, this
        // limitation could be lifted in the future.
        if attrs_excluded(&i.attrs) {
            trace!("match excluded by attrs");
            return;
        }

        let has_catchall = i
            .arms
            .iter()
            .any(|arm| matches!(arm.pat, syn::Pat::Wild(_)));
        if has_catchall {
            i.arms
                .iter()
                // Don't mutate the wild arm, because that will likely be unviable, and also
                // skip it if a guard is present, because the replacement of the guard with 'false'
                // below is logically equivalent to removing the arm.
                .filter(|arm| !matches!(arm.pat, syn::Pat::Wild(_)) && arm.guard.is_none())
                .for_each(|arm| {
                    self.collect_mutant(arm.span().into(), &quote! {}, Genre::MatchArm);
                });
        } else {
            trace!("match has no `_` pattern");
        }

        i.arms
            .iter()
            .flat_map(|arm| &arm.guard)
            .for_each(|(_if, guard_expr)| {
                self.collect_mutant(
                    guard_expr.span().into(),
                    &quote! { true },
                    Genre::MatchArmGuard,
                );
                self.collect_mutant(
                    guard_expr.span().into(),
                    &quote! { false },
                    Genre::MatchArmGuard,
                );
            });

        syn::visit::visit_expr_match(self, i);
    }
}

// Get the span of the block excluding the braces, or None if it is empty.
fn function_body_span(block: &Block) -> Option<Span> {
    Some(Span {
        start: block.stmts.first()?.span().start().into(),
        end: block.stmts.last()?.span().end().into(),
    })
}

/// Find a new source file referenced by a `mod` statement.
///
/// Possibly, our heuristics just won't be able to find which file it is,
/// in which case we return `Ok(None)`.
fn find_mod_source(
    tree_root: &Utf8Path,
    parent: &SourceFile,
    mod_namespace: &ExternalModRef,
) -> Option<Utf8PathBuf> {
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

    let (mod_child, mod_parents) = mod_namespace
        .parts
        .split_last()
        .expect("mod namespace is empty");

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
            return Some(relative_path);
        }
        tried_paths.push(full_path);
    }
    let mod_name = &mod_child.name;
    let definition_site = parent.format_source_location(mod_child.source_location.start);
    warn!(?definition_site, %mod_name, ?tried_paths, "referent of mod not found");
    None
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
            attr = attr.to_pretty_string(),
            "Attribute is in an unrecognized form so skipped",
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

/// True if the path ends with this identifier.
///
/// This is used as a heuristic to match types without being sensitive to which
/// module they are in, or to match functions without being sensitive to which
/// type they might be associated with.
///
/// This does not check type arguments.
fn path_ends_with(path: &syn::Path, ident: &str) -> bool {
    path.segments.last().is_some_and(|s| s.ident == ident)
}

/// True if the attribute contains `mutants::skip`.
///
/// This for example returns true for `#[mutants::skip]` or `#[cfg_attr(test, mutants::skip)]`.
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
            skip = true;
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
    use pretty_assertions::assert_eq;
    use test_log::test;

    use crate::test_util::copy_of_testdata;
    use crate::workspace::{PackageFilter, Workspace};

    use super::*;

    #[test]
    fn path_ends_with() {
        use super::path_ends_with;
        use syn::parse_quote;

        let path = parse_quote! { foo::bar::baz };
        assert!(path_ends_with(&path, "baz"));
        assert!(!path_ends_with(&path, "bar"));
        assert!(!path_ends_with(&path, "foo"));

        let path = parse_quote! { baz };
        assert!(path_ends_with(&path, "baz"));
        assert!(!path_ends_with(&path, "bar"));

        let path = parse_quote! { BTreeMap<K, V> };
        assert!(path_ends_with(&path, "BTreeMap"));
        assert!(!path_ends_with(&path, "V"));
        assert!(!path_ends_with(&path, "K"));
    }

    /// We should not generate mutants that produce the same tokens as the
    /// source.
    #[test]
    fn no_mutants_equivalent_to_source() {
        let code = indoc! { "
            fn always_true() -> bool { true }
        "};
        let source_file = SourceFile::for_tests("src/lib.rs", code, "unimportant", true);
        let (mutants, _files) =
            walk_file(&source_file, &[], &Options::default()).expect("walk_file");
        let mutant_names = mutants.iter().map(|m| m.name(false)).collect_vec();
        // It would be good to suggest replacing this with 'false', breaking a key behavior,
        // but bad to replace it with 'true', changing nothing.
        assert_eq!(
            mutant_names,
            ["src/lib.rs: replace always_true -> bool with false"]
        );
    }

    /// We don't visit functions inside files marked with `#![cfg(test)]`.
    #[test]
    fn no_mutants_in_files_with_inner_cfg_test_attribute() {
        let options = Options::default();
        let console = Console::new();
        let tmp = copy_of_testdata("cfg_test_inner");
        let workspace = Workspace::open(tmp.path()).unwrap();
        let discovered = workspace
            .discover(&PackageFilter::All, &options, &console)
            .unwrap();
        assert_eq!(discovered.mutants.as_slice(), &[]);
    }

    /// Helper function for `find_path_attribute` tests
    fn run_find_path_attribute(
        token_stream: &TokenStream,
    ) -> std::result::Result<Option<Utf8PathBuf>, String> {
        let token_string = token_stream.to_string();
        let item_mod = syn::parse_str::<syn::ItemMod>(&token_string).unwrap_or_else(|err| {
            panic!("Failed to parse test case token stream: {token_string}\n{err}")
        });
        find_path_attribute(&item_mod.attrs)
    }

    #[test]
    fn find_path_attribute_on_module_item() {
        let outer = run_find_path_attribute(&quote! {
            #[path = "foo_file.rs"]
            mod foo;
        });
        assert_eq!(outer, Ok(Some(Utf8PathBuf::from("foo_file.rs"))));

        let inner = run_find_path_attribute(&quote! {
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
        let dots = run_find_path_attribute(&quote! {
            #[path = "contains/../dots.rs"]
            mod dots;
        });
        assert_eq!(dots, Ok(Some(Utf8PathBuf::from("contains/../dots.rs"))));

        let dots_inner = run_find_path_attribute(&quote! {
            mod dots_in_path {
                #![path = "contains/../dots"]
            }
        });
        assert_eq!(dots_inner, Ok(Some(Utf8PathBuf::from("contains/../dots"))));

        let leading_slash = run_find_path_attribute(&quote! {
            #[path = "/leading_slash.rs"]
            mod dots;
        });
        assert_eq!(leading_slash, Err("/leading_slash.rs".to_owned()));

        let allow_other_slashes = run_find_path_attribute(&quote! {
            #[path = "foo/other/slashes/are/allowed.rs"]
            mod dots;
        });
        assert_eq!(
            allow_other_slashes,
            Ok(Some(Utf8PathBuf::from("foo/other/slashes/are/allowed.rs")))
        );

        let leading_slash2 = run_find_path_attribute(&quote! {
            #[path = "/leading_slash/../and_dots.rs"]
            mod dots;
        });
        assert_eq!(
            leading_slash2,
            Err("/leading_slash/../and_dots.rs".to_owned())
        );
    }

    /// Demonstrate that we can generate mutants from a string, without needing a whole tree.
    #[test]
    fn mutants_from_test_str() {
        let options = Options::default();
        let mutants = mutate_source_str(
            indoc! {"
                fn always_true() -> bool { true }
            "},
            &options,
        )
        .expect("walk_file_string");
        assert_eq!(
            mutants.iter().map(|m| m.name(false)).collect_vec(),
            ["src/main.rs: replace always_true -> bool with false"]
        );
    }

    /// Skip mutating arguments to a particular named function.
    #[test]
    fn skip_named_fn() {
        let options = Options {
            skip_calls: vec!["dont_touch_this".to_owned()],
            ..Default::default()
        };
        let mut mutants = mutate_source_str(
            indoc! {"
                fn main() {
                    dont_touch_this(2 + 3);
                }
            "},
            &options,
        )
        .expect("walk_file_string");
        // Ignore the main function itself
        mutants.retain(|m| m.genre != Genre::FnValue);
        assert_eq!(mutants, []);
    }

    #[test]
    fn skip_with_capacity_by_default() {
        let options = Options::from_arg_strs(["mutants"]);
        let mut mutants = mutate_source_str(
            indoc! {"
                fn main() {
                    let mut v = Vec::with_capacity(2 * 100);
                }
            "},
            &options,
        )
        .expect("walk_file_string");
        // Ignore the main function itself
        mutants.retain(|m| m.genre != Genre::FnValue);
        assert_eq!(mutants, []);
    }

    #[test]
    fn mutate_vec_with_capacity_when_default_skips_are_turned_off() {
        let options = Options::from_arg_strs(["mutants", "--skip-calls-defaults", "false"]);
        let mutants = mutate_source_str(
            indoc! {"
                fn main() {
                    let mut _v = std::vec::Vec::<String>::with_capacity(2 * 100);
                }
            "},
            &options,
        )
        .expect("walk_file_string");
        dbg!(&mutants);
        // The main fn plus two mutations of the `*` expression.
        assert_eq!(mutants.len(), 3);
    }

    #[test]
    fn skip_method_calls_by_name() {
        let options = Options::from_arg_strs(["mutants", "--skip-calls", "dont_touch_this"]);
        let mutants = mutate_source_str(
            indoc! {"
                fn main() {
                    let mut v = v::new();
                    v.dont_touch_this(2 + 3);
                }
            "},
            &options,
        )
        .unwrap();
        dbg!(&mutants);
        assert_eq!(
            mutants
                .iter()
                .filter(|mutant| mutant.genre != Genre::FnValue)
                .count(),
            0
        );
    }

    #[test]
    fn mutant_name_includes_type_parameters() {
        // From https://github.com/sourcefrog/cargo-mutants/issues/334
        let options = Options::from_arg_strs(["mutants"]);
        let mutants = mutate_source_str(
            indoc! {r#"
            impl AsRef<str> for Apath {
                fn as_ref(&self) -> &str {
                    &self.0
                }
            }

            impl From<Apath> for String {
                fn from(a: Apath) -> String {
                    a.0
                }
            }

            impl<'a> From<&'a str> for Apath {
                fn from(s: &'a str) -> Apath {
                    assert!(Apath::is_valid(s), "invalid apath: {s:?}");
                    Apath(s.to_string())
                }
            }
            "#},
            &options,
        )
        .unwrap();
        dbg!(&mutants);
        let mutant_names = mutants.iter().map(|m| m.name(false) + "\n").join("");
        assert_eq!(
            mutant_names,
            indoc! {r#"
                src/main.rs: replace <impl AsRef<str> for Apath>::as_ref -> &str with ""
                src/main.rs: replace <impl AsRef<str> for Apath>::as_ref -> &str with "xyzzy"
                src/main.rs: replace <impl From<Apath> for String>::from -> String with String::new()
                src/main.rs: replace <impl From<Apath> for String>::from -> String with "xyzzy".into()
                src/main.rs: replace <impl From<&'a str> for Apath>::from -> Apath with Default::default()
            "#}
        );
    }

    #[test]
    fn mutate_match_arms_with_fallback() {
        let options = Options::default();
        let mutants = mutate_source_str(
            indoc! {"
                fn main() {
                    match x {
                        X::A => {},
                        X::B => {},
                        _ => {},
                    }
                }
            "},
            &options,
        )
        .unwrap();
        assert_eq!(
            mutants
                .iter()
                .filter(|m| m.genre == Genre::MatchArm)
                .map(|m| m.name(true))
                .collect_vec(),
            [
                "src/main.rs:3:9: delete match arm",
                "src/main.rs:4:9: delete match arm",
            ]
        );
    }

    #[test]
    fn skip_match_arms_without_fallback() {
        let options = Options::default();
        let mutants = mutate_source_str(
            indoc! {"
                fn main() {
                    match x {
                        X::A => {},
                        X::B => {},
                    }
                }
            "},
            &options,
        )
        .unwrap();

        let empty: &[&str] = &[];
        assert_eq!(
            mutants
                .iter()
                .filter(|m| m.genre == Genre::MatchArm)
                .map(|m| m.name(true))
                .collect_vec(),
            empty
        );
    }

    #[test]
    fn mutate_match_guard() {
        let options = Options::default();
        let mutants = mutate_source_str(
            indoc! {"
                fn main() {
                    match x {
                        X::A if foo() => {},
                        X::A => {},
                        X::B => {},
                        X::C if bar() => {},
                    }
                }
            "},
            &options,
        )
        .unwrap();
        assert_eq!(
            mutants
                .iter()
                .filter(|m| m.genre == Genre::MatchArmGuard)
                .map(|m| m.name(true))
                .collect_vec(),
            [
                "src/main.rs:3:17: replace match guard with true",
                "src/main.rs:3:17: replace match guard with false",
                "src/main.rs:6:17: replace match guard with true",
                "src/main.rs:6:17: replace match guard with false",
            ]
        );
    }

    #[test]
    fn skip_removing_match_arm_with_guard() {
        let options = Options::default();
        let mutants = mutate_source_str(
            indoc! {"
                fn main() {
                    match x {
                        X::A if foo() => {},
                        X::A => {},
                        _ => {},
                    }
                }
            "},
            &options,
        )
        .unwrap();
        assert_eq!(
            mutants
                .iter()
                .filter(|m| matches!(m.genre, Genre::MatchArmGuard | Genre::MatchArm))
                .map(|m| m.name(true))
                .collect_vec(),
            [
                "src/main.rs:4:9: delete match arm",
                "src/main.rs:3:17: replace match guard with true",
                "src/main.rs:3:17: replace match guard with false",
            ]
        );
    }
}

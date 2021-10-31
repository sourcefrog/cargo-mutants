// Copyright 2021 Martin Pool

//! Mutations to source files, and inference of interesting mutations to apply.

use std::fmt;
use std::fs;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use serde::ser::{SerializeStruct, Serializer};
use serde::Serialize;
use similar::TextDiff;
use syn::Attribute;
use syn::ItemFn;
// use syn::parse;
// use quote::ToTokens;
use syn::visit::Visit;

use crate::source::SourceFile;
use crate::textedit::{replace_region, Span};

/// A comment marker inserted next to changes, so they can be easily found.
const MUTATION_MARKER_COMMENT: &str = "/* ~ changed by cargo-mutants ~ */";

/// A type of mutation operation that could be applied to a source file.
#[derive(Debug, Eq, PartialEq, Serialize)]
pub enum MutationOp {
    /// Return [Default::default].
    Default,
    /// Replace the function body with nothing (for functions that return `()`.
    ///
    /// We use `()` rather than just nothing because it's clearer in messages about the mutation.
    Unit,
    /// Return true.
    True,
    /// Return false.
    False,
    /// Return empty string.
    EmptyString,
    /// Return `"xyzzy"`.
    Xyzzy,
}

impl MutationOp {
    /// Return the text that replaces the body of the mutated span, without the marker comment.
    fn replacement(&self) -> &'static str {
        use MutationOp::*;
        match self {
            Default => "Default::default()",
            Unit => "()",
            True => "true",
            False => "false",
            EmptyString => "\"\".into()",
            Xyzzy => "\"xyzzy\".into()",
        }
    }
}

/// A mutation that could possibly be applied to source code.
///
/// The Mutation knows:
/// * which file to modify,
/// * which function and span in that file,
/// * and what type of mutation to apply.
pub struct Mutation {
    pub source_file: SourceFile,

    /// The function that's being mutated.
    function_name: String,

    /// The mutated textual region.
    span: Span,

    /// The type of change to apply.
    pub op: MutationOp,
}

impl Mutation {
    /// Return text of the whole file with the mutation applied.
    pub fn mutated_code(&self) -> String {
        replace_region(
            &self.source_file.code,
            &self.span.start,
            &self.span.end,
            &format!(
                "{{\n{} {}\n}}\n",
                self.op.replacement(),
                MUTATION_MARKER_COMMENT
            ),
        )
    }

    /// Return the original code for the entire file affected by this mutation.
    pub fn original_code(&self) -> &str {
        &self.source_file.code
    }

    /// Return a "file:line" description of the location of this mutation.
    pub fn describe_location(&self) -> String {
        format!(
            "{}:{}",
            self.source_file.tree_relative_slashes(),
            self.span.start.line,
        )
    }

    /// Describe the mutation briefly, not including the location.
    pub fn describe_change(&self) -> String {
        format!(
            "replace {} with {}",
            self.function_name(),
            self.op.replacement()
        )
    }

    /// Return the name of the function to be mutated.
    ///
    /// Note that this will often not be unique: the same name can be reused
    /// in different modules, under different cfg guards, etc.
    pub fn function_name(&self) -> &str {
        &self.function_name
    }

    /// Return a unified diff for the mutation.
    pub fn diff(&self) -> String {
        let old_label = self.source_file.tree_relative_slashes();
        let new_label = self.describe_change();
        TextDiff::from_lines(self.original_code(), &self.mutated_code())
            .unified_diff()
            .context_radius(8)
            .header(&old_label, &new_label)
            .to_string()
    }

    /// Change the file affected by this mutation in the given directory.
    pub fn apply_in_dir(&self, dir: &Path) -> Result<()> {
        self.write_in_dir(dir, &self.mutated_code())
    }

    /// Restore the file affected by this mutation to its original text.
    pub fn revert_in_dir(&self, dir: &Path) -> Result<()> {
        self.write_in_dir(dir, self.original_code())
    }

    fn write_in_dir(&self, dir: &Path, code: &str) -> Result<()> {
        let path = self.source_file.within_dir(dir);
        // for safety, don't follow symlinks
        assert!(path.is_file(), "{:?} is not a file", path);
        fs::write(&path, code.as_bytes())
            .with_context(|| format!("failed to write mutated code to {:?}", path))
    }
}

impl fmt::Debug for Mutation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Mutation")
            .field("op", &self.op)
            .field("function_name", &self.function_name())
            // more concise display of spans
            .field("start", &(self.span.start.line, self.span.start.column))
            .field("end", &(self.span.end.line, self.span.end.column))
            .finish()
    }
}

impl fmt::Display for Mutation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} in {}",
            self.describe_change(),
            self.describe_location(),
        )
    }
}

impl Serialize for Mutation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // custom serialize to omit inessential info
        let mut ss = serializer.serialize_struct("Mutation", 4)?;
        ss.serialize_field("file", &self.source_file.tree_relative_slashes())?;
        ss.serialize_field("line", &self.span.start.line)?;
        ss.serialize_field("function", &self.function_name)?;
        ss.serialize_field("replacement", self.op.replacement())?;
        ss.end()
    }
}

/// `syn` visitor that recursively traverses the syntax tree, accumulating places that could be mutated.
pub(crate) struct DiscoveryVisitor<'sf> {
    /// All the mutations generated by visiting the file.
    pub(crate) mutations: Vec<Mutation>,

    /// The file being visited.
    source_file: &'sf SourceFile,

    /// The stack of namespaces we're currently inside.
    namespace_stack: Vec<String>,
}

impl<'sf> DiscoveryVisitor<'sf> {
    pub(crate) fn new(source_file: &'sf SourceFile) -> DiscoveryVisitor<'sf> {
        DiscoveryVisitor {
            source_file,
            mutations: Vec::new(),
            namespace_stack: Vec::new(),
        }
    }

    fn collect_mutation(&mut self, op:MutationOp, item_fn: &ItemFn) {
        self.namespace_stack.push(item_fn.sig.ident.to_string());
        let function_name = self.namespace_stack.join("::");
        self.mutations.push(Mutation {
            source_file: self.source_file.clone(),
            op,
            function_name,
            span: item_fn.block.brace_token.span.into(),
        });
        self.namespace_stack.pop();
    }
}

impl<'ast, 'sf> Visit<'ast> for DiscoveryVisitor<'sf> {
    // TODO: Also visit methods and maybe closures.

    fn visit_item_fn(&mut self, item_fn: &'ast ItemFn) {
        // TODO: Filter out more inapplicable fns.
        if attrs_excluded(&item_fn.attrs) {
            return; // don't look inside it either
        }
        // Look at the return type and try to work out what values might be valid to return.
        let mut ops: Vec<MutationOp> = Vec::new();
        match &item_fn.sig.output {
            syn::ReturnType::Default => ops.push(MutationOp::Unit),
            syn::ReturnType::Type(_rarrow, box_typ) => match &**box_typ {
                syn::Type::Path(syn::TypePath { path, .. }) => {
                    if path.is_ident("bool") {
                        ops.push(MutationOp::True);
                        ops.push(MutationOp::False);
                    } else if path.is_ident("String") {
                        // TODO: Detect &str etc.
                        ops.push(MutationOp::EmptyString);
                        ops.push(MutationOp::Xyzzy);
                    } else {
                        ops.push(MutationOp::Default)
                    }
                }
                _ => ops.push(MutationOp::Default),
            },
        }
        ops.into_iter().for_each(|op| self.collect_mutation(op, item_fn));
        syn::visit::visit_item_fn(self, item_fn);
    }

    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        // TODO: Remember which mods we're inside, and put the path into the name of visited
        // functions.
        if !attrs_excluded(&node.attrs) {
            let name = node.ident.to_string();
            self.namespace_stack.push(name.clone());
            syn::visit::visit_item_mod(self, node);
            assert_eq!(self.namespace_stack.pop(), Some(name));
        }
    }
}

/// True if any of the attrs indicate that we should skip this node and everything inside it.
fn attrs_excluded(attrs: &[Attribute]) -> bool {
    attrs
        .iter()
        .any(|attr| attr_is_cfg_test(attr) || attr_is_test(attr) || attr_is_mutants_skip(attr))
}

/// True if the attribute is `#[cfg(test)]`.
fn attr_is_cfg_test(attr: &Attribute) -> bool {
    if !attr.path.is_ident("cfg") {
        return false;
    }
    if let syn::Meta::List(meta_list) = attr.parse_meta().unwrap() {
        // We should have already checked this above, but to make sure:
        assert!(meta_list.path.is_ident("cfg"));
        for nested_meta in meta_list.nested {
            if let syn::NestedMeta::Meta(syn::Meta::Path(cfg_path)) = nested_meta {
                if cfg_path.is_ident("test") {
                    return true;
                }
            }
        }
    }
    false
}

/// True if the attribute is `#[test]`.
fn attr_is_test(attr: &Attribute) -> bool {
    attr.path.is_ident("test")
}

/// True if the attribute is `#[mutants::skip]`.
fn attr_is_mutants_skip(attr: &Attribute) -> bool {
    attr.path
        .segments
        .iter()
        .map(|ps| &ps.ident)
        .eq(["mutants", "skip"].iter())
}

#[cfg(test)]
mod test {
    use std::path::Path;

    use itertools::Itertools;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn discover_mutations() {
        let source_file = SourceFile::new(
            &Path::new("testdata/tree/factorial"),
            &Path::new("src/bin/main.rs"),
        )
        .unwrap();
        let muts = source_file.mutations().unwrap();
        assert_eq!(muts.len(), 2);
        assert_eq!(
            format!("{:?}", muts[0]),
            "Mutation { op: Unit, function_name: \"main\", start: (1, 11), end: (5, 2) }"
        );
        assert_eq!(
            format!("{:?}", muts[1]),
            "Mutation { op: Default, function_name: \"factorial\", start: (7, 29), end: (13, 2) }"
        );
    }

    #[test]
    fn filter_by_attributes() {
        let source_file = SourceFile::new(
            &Path::new("testdata/tree/could_hang"),
            &Path::new("src/lib.rs"),
        )
        .unwrap();
        let muts = source_file.mutations().unwrap();
        let descriptions = muts.iter().map(Mutation::describe_change).collect_vec();
        insta::assert_snapshot!(
            descriptions.join("\n"),
            @"replace controlled_loop with ()"
        );
    }

    #[test]
    fn mutate_factorial() {
        let source_file = SourceFile::new(
            Path::new("testdata/tree/factorial"),
            &Path::new("src/bin/main.rs"),
        )
        .unwrap();
        let muts = source_file.mutations().unwrap();
        assert_eq!(muts.len(), 2);

        let mut mutated_code = muts[0].mutated_code();
        assert_eq!(muts[0].function_name(), "main");
        mutated_code.retain(|c| c != '\r');
        assert_eq!(
            mutated_code,
            r#"fn main() {
() /* ~ changed by cargo-mutants ~ */
}

fn factorial(n: u32) -> u32 {
    let mut a = 1;
    for i in 2..=n {
        a *= i;
    }
    a
}

#[test]
fn test_factorial() {
    assert_eq!(factorial(6), 720);
}
"#
        );

        let mut mutated_code = muts[1].mutated_code();
        assert_eq!(muts[1].function_name(), "factorial");
        mutated_code.retain(|c| c != '\r');
        assert_eq!(
            mutated_code,
            r#"fn main() {
    for i in 1..=6 {
        println!("{}! = {}", i, factorial(i));
    }
}

fn factorial(n: u32) -> u32 {
Default::default() /* ~ changed by cargo-mutants ~ */
}

#[test]
fn test_factorial() {
    assert_eq!(factorial(6), 720);
}
"#
        );
    }
}

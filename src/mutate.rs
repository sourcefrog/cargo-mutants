// Copyright 2021 Martin Pool

//! Mutate source files.

use std::fmt;
use std::fs;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use proc_macro2::Span;
use similar::TextDiff;
use syn::Attribute;
use syn::ItemFn;
// use syn::parse;
// use quote::ToTokens;
use syn::visit::Visit;

use crate::source::SourceFile;
use crate::textedit::replace_region;

/// A comment marker inserted next to changes, so they can be easily found.
const MUTATION_MARKER_COMMENT: &str = "/* ~ changed by cargo-mutants ~ */";

/// A type of mutation operation that could be applied to a source file.
#[derive(Debug, Eq, PartialEq)]
pub enum MutationOp {
    /// Return [Default::default].
    ReturnDefault,
}

impl MutationOp {
    /// Return the text that replaces the body of the mutated span, without the marker comment.
    fn replacement(&self) -> &'static str {
        use MutationOp::*;
        match self {
            ReturnDefault => "Default::default()",
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
    // TODO: Generalize to mutations that don't replace a whole function.
    pub source_file: SourceFile,
    pub op: MutationOp,
    function_ident: syn::Ident,
    span: Span,
}

impl Mutation {
    /// Return text of the whole file with the mutation applied.
    pub fn mutated_code(&self) -> String {
        replace_region(
            &self.source_file.code,
            &self.span.start(),
            &self.span.end(),
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
        let start = self.span.start();
        format!(
            "{}:{}",
            self.source_file.tree_relative_slashes(),
            start.line,
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
    pub fn function_name(&self) -> String {
        self.function_ident.to_string()
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
            .field("function", &self.function_ident.to_string())
            .field("start", &(self.span.start().line, self.span.start().column))
            .field("end", &(self.span.end().line, self.span.end().column))
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

pub(crate) struct DiscoveryVisitor<'sf> {
    pub(crate) sites: Vec<Mutation>,
    source_file: &'sf SourceFile,
}

impl<'sf> DiscoveryVisitor<'sf> {
    pub(crate) fn new(source_file: &'sf SourceFile) -> DiscoveryVisitor<'sf> {
        DiscoveryVisitor {
            source_file,
            sites: Vec::new(),
        }
    }
}

impl<'ast, 'sf> Visit<'ast> for DiscoveryVisitor<'sf> {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        // TODO: Filter out inapplicable fns.
        // TODO: Also visit methods and maybe closures.
        if attrs_include_test(&node.attrs) || attrs_include_mutants_skip(&node.attrs) {
            return; // don't look inside it either
        }
        self.sites.push(Mutation {
            source_file: self.source_file.clone(),
            op: MutationOp::ReturnDefault,
            function_ident: node.sig.ident.clone(),
            span: node.block.brace_token.span,
        });
        syn::visit::visit_item_fn(self, node);
    }
}

fn attrs_include_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path
            .segments
            .iter()
            .map(|ps| &ps.ident)
            .eq(["test"].iter())
    })
}

fn attrs_include_mutants_skip(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path
            .segments
            .iter()
            .map(|ps| &ps.ident)
            .eq(["mutants", "skip"].iter())
    })
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
            "Mutation { op: ReturnDefault, function: \"main\", start: (1, 10), end: (5, 1) }"
        );
        assert_eq!(
            format!("{:?}", muts[1]),
            "Mutation { op: ReturnDefault, function: \"factorial\", start: (7, 28), end: (13, 1) }"
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
            @"replace controlled_loop with Default::default()"
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
Default::default() /* ~ changed by cargo-mutants ~ */
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

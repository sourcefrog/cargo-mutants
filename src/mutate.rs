// Copyright 2021 Martin Pool

use std::fmt;
#[allow(unused)]
use std::path::PathBuf;

#[allow(unused)]
use anyhow::Result;
use proc_macro2::Span;
use similar::TextDiff;
use syn::ItemFn;
// use syn::parse;
// use quote::ToTokens;
use syn::visit::Visit;

use crate::source::SourceFile;
use crate::textedit::replace_region;

#[derive(Debug, Eq, PartialEq)]
pub enum MutationOp {
    ReturnDefault,
}

#[derive()]
pub struct Mutation<'a> {
    pub source_file: &'a SourceFile,
    pub op: MutationOp,
    function_ident: syn::Ident,
    span: Span,
}

impl<'a> Mutation<'a> {
    pub fn mutated_code(&self) -> String {
        match self.op {
            MutationOp::ReturnDefault => replace_region(
                &self.source_file.code,
                &self.span.start(),
                &self.span.end(),
                "{\n/* ~ removed by enucleate ~ */ Default::default()\n}\n",
            ),
        }
    }

    pub fn original_code(&self) -> &str {
        &self.source_file.code
    }

    /// Return a "file:line:column" description of the location of this mutation.
    ///
    /// Columns are expressed 1-based which seems more common in editors.
    pub fn describe_location(&self) -> String {
        let start = self.span.start();
        format!(
            "{}:{}:{}",
            self.source_file.tree_relative_slashes(),
            start.line,
            start.column + 1
        )
    }

    /// Describe the mutation briefly, not including the location.
    pub fn describe_change(&self) -> String {
        match self.op {
            MutationOp::ReturnDefault => {
                format!("replace {} with Default::default()", self.function_name())
            }
        }
    }

    /// Return the name of the function to be mutated.
    ///
    /// Note that this will often not be unique: the same name can be reused
    /// in different modules, under different cfg guards, etc.
    #[allow(unused)]
    pub fn function_name(&self) -> String {
        self.function_ident.to_string()
    }

    /// Return a unified diff for the mutation.
    pub fn diff(&self) -> String {
        let old_label = self.source_file.tree_relative_slashes();
        let new_label = self.describe_change();
        let mutated_code = self.mutated_code();
        let text_diff = TextDiff::from_lines(self.original_code(), &mutated_code);
        text_diff
            .unified_diff()
            .context_radius(8)
            .header(&old_label, &new_label)
            .to_string()
    }
}

impl<'sf> fmt::Debug for Mutation<'sf> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Mutation")
            .field("op", &self.op)
            .field("function", &self.function_ident.to_string())
            .field("start", &(self.span.start().line, self.span.start().column))
            .field("end", &(self.span.end().line, self.span.end().column))
            .finish()
    }
}

pub(crate) struct DiscoveryVisitor<'sf> {
    pub(crate) sites: Vec<Mutation<'sf>>,
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
        if item_fn_is_test(node) {
            // eprintln!("skip #[test] fn {:?}", node.sig.ident);
            return; // don't look inside it either
        }
        self.sites.push(Mutation {
            source_file: self.source_file,
            op: MutationOp::ReturnDefault,
            function_ident: node.sig.ident.clone(),
            span: node.block.brace_token.span,
        });
        // let span = &node.block.brace_token.span;
        // eprintln!(
        //     "visit item fn {} with brace token span {:?}-{:?} {:#?}",
        //     node.sig.ident,
        //     span.start(),
        //     span.end(),
        //     span.start(),
        // );
        syn::visit::visit_item_fn(self, node);
    }
}

fn item_fn_is_test(node: &ItemFn) -> bool {
    node.attrs.iter().any(|attr| {
        attr.path
            .segments
            .iter()
            .map(|ps| &ps.ident)
            .eq(["test"].iter())
    })
}

#[cfg(test)]
mod test {
    use std::path::Path;

    use pretty_assertions::assert_eq;

    use crate::source::SourceTree;

    #[allow(unused)]
    use super::*;

    #[test]
    fn discover_mutations() {
        let source_tree = SourceTree::new(&Path::new("testdata/tree/factorial")).unwrap();
        let source_file = source_tree
            .source_file(&Path::new("src/bin/main.rs"))
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
    fn mutate_factorial() {
        let source_tree = SourceTree::new(&Path::new("testdata/tree/factorial")).unwrap();
        let source_file = source_tree
            .source_file(&Path::new("src/bin/main.rs"))
            .unwrap();
        let muts = source_file.mutations().unwrap();
        assert_eq!(muts.len(), 2);

        let mut mutated_code = muts[0].mutated_code();
        assert_eq!(muts[0].function_name(), "main");
        mutated_code.retain(|c| c != '\r');
        assert_eq!(
            mutated_code,
            r#"fn main() {
/* ~ removed by enucleate ~ */ Default::default()
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
/* ~ removed by enucleate ~ */ Default::default()
}

#[test]
fn test_factorial() {
    assert_eq!(factorial(6), 720);
}
"#
        );
    }
}

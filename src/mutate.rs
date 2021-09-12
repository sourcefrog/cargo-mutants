// Copyright 2021 Martin Pool

use std::fmt;
use std::path::PathBuf;

use anyhow::Result;
use proc_macro2::Span;
use syn::ItemFn;
// use syn::parse;
// use quote::ToTokens;
use syn::visit::Visit;

use crate::textedit::replace_region;

#[derive(Debug, Eq, PartialEq)]
pub enum MutationOp {
    ReturnDefault,
}

#[derive()]
pub struct Mutation {
    pub op: MutationOp,
    function_ident: syn::Ident,
    span: Span,
}

impl Mutation {
    pub fn mutated_code(&self, mutagen: &FileMutagen) -> String {
        match self.op {
            MutationOp::ReturnDefault => replace_region(
                &mutagen.code,
                &self.span.start(),
                &self.span.end(),
                "{\n/* ~ removed by enucleate ~ */ Default::default()\n}\n",
            ),
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

#[derive(Default, Debug)]
struct DiscoveryVisitor {
    sites: Vec<Mutation>,
}

impl<'ast> Visit<'ast> for DiscoveryVisitor {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        // TODO: Filter out inapplicable fns.
        // TODO: Also visit methods and maybe closures.
        if item_fn_is_test(node) {
            eprintln!("skip #[test] fn {:?}", node.sig.ident);
            return; // don't look inside it either
        }
        self.sites.push(Mutation {
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

pub struct FileMutagen {
    #[allow(unused)]
    path: PathBuf,
    code: String,
    syn_file: syn::File,
}

impl FileMutagen {
    pub fn new(path: PathBuf) -> Result<FileMutagen> {
        let code = std::fs::read_to_string(&path)?;
        let syn_file = syn::parse_str::<syn::File>(&code)?;
        Ok(FileMutagen {
            path,
            code,
            syn_file,
        })
    }

    pub fn discover_mutation_sites(&self) -> Vec<Mutation> {
        let mut v = DiscoveryVisitor::default();
        v.visit_file(&self.syn_file);
        v.sites
    }
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn discover_mutations() {
        let mutagen = FileMutagen::new("testdata/tree/factorial/src/bin/main.rs".into()).unwrap();
        let muts = mutagen.discover_mutation_sites();
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
        let mutagen = FileMutagen::new("testdata/tree/factorial/src/bin/main.rs".into()).unwrap();
        let muts = mutagen.discover_mutation_sites();
        assert_eq!(muts.len(), 2);

        let mut mutated_code = muts[0].mutated_code(&mutagen);
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

        let mut mutated_code = muts[1].mutated_code(&mutagen);
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

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
    op: MutationOp,
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
                "{\n /* ~ removed by enucleate ~ */ Default::default()\n}",
            ),
        }
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
}

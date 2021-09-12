// Copyright 2021 Martin Pool

use std::env::args;
use std::path::PathBuf;

use anyhow::Result;
use proc_macro2::Span;
use syn::ItemFn;
// use syn::parse;
// use quote::ToTokens;
use syn::visit::Visit;

mod textedit;
use textedit::replace_region;

#[derive(Debug, Eq, PartialEq)]
enum MutationOp {
    ReturnDefault,
}

#[derive(Debug)]
struct Mutation {
    op: MutationOp,
    function_ident: syn::Ident,
    span: Span,
    // We could later have a concept of mutations that apply at scopes other
    // than whole functions.
}

impl Mutation {
    fn apply(&self, source: &str) -> String {
        match self.op {
            MutationOp::ReturnDefault => replace_region(
                source,
                &self.span.start(),
                &self.span.end(),
                "{\n /* ~ removed by enucleate ~ */ Default::default()\n}",
            ),
        }
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
            span: node.block.brace_token.span.clone(),
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

struct FileMutagen {
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

    fn discover_mutation_sites(&self) -> Vec<Mutation> {
        let mut v = DiscoveryVisitor::default();
        v.visit_file(&self.syn_file);
        v.sites
    }
}

fn main() -> Result<()> {
    let srcpath = PathBuf::from(&args().nth(1).expect("a Rust source file name"));
    let mutagen = FileMutagen::new(srcpath)?;
    let mutation_sites = mutagen.discover_mutation_sites();
    // eprintln!("{:#?}", mutation_sites);
    for m in &mutation_sites[..1] {
        print!("{}", m.apply(&mutagen.code));
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn discover_mutations() {
        let mutagen = FileMutagen::new("testdata/tree/factorial/src/bin/main.rs".into()).unwrap();
        let muts = mutagen.discover_mutation_sites();
        assert_eq!(muts.len(), 2);
    }
}

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
use textedit::replace_line_column_region;

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
            MutationOp::ReturnDefault => replace_line_column_region(
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

fn discover_mutation_sites(file_ast: &syn::File) -> Vec<Mutation> {
    let mut v = DiscoveryVisitor::default();
    v.visit_file(&file_ast);
    v.sites
}

fn main() -> Result<()> {
    let srcpath = PathBuf::from(&args().nth(1).expect("a Rust source file name"));

    let code = std::fs::read_to_string(&srcpath)?;
    let file_ast = syn::parse_str::<syn::File>(&code)?;
    // println!("{:#?}", expr);
    let mutation_sites = discover_mutation_sites(&file_ast);
    // eprintln!("{:#?}", mutation_sites);
    for m in &mutation_sites[..1] {
        print!("{}", m.apply(&code));
    }
    // let out_tokens = file_ast.into_token_stream();
    // println!("{}", out_tokens);
    Ok(())
}

#[cfg(test)]
mod test {
    // use super::*;
}

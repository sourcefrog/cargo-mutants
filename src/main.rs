// Copyright 2021 Martin Pool

use std::env::args;
use std::path::PathBuf;

use anyhow::Result;
// use syn::parse;
use syn::visit::Visit;
use syn::ItemFn;

struct FnVisitor {}

impl<'ast> Visit<'ast> for FnVisitor {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        let span = &node.block.brace_token.span;
        println!(
            "visit item fn {} with brace token span {:?}-{:?}",
            node.sig.ident,
            span.start(),
            span.end(),
        );
        syn::visit::visit_item_fn(self, node);
    }
}

fn main() -> Result<()> {
    let srcpath = PathBuf::from(&args().nth(1).expect("a Rust source file name"));

    let code = std::fs::read_to_string(&srcpath)?;
    let file_ast = syn::parse_str::<syn::File>(&code)?;
    // println!("{:#?}", expr);
    FnVisitor {}.visit_file(&file_ast);
    Ok(())
}

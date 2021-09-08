// Copyright 2021 Martin Pool

use std::env::args;
use std::path::PathBuf;

use anyhow::{anyhow, Result};

use syntex_syntax::ast::{FnDecl, NodeId};
use syntex_syntax::codemap::FilePathMapping;
use syntex_syntax::ext::quote::rt::Span;
use syntex_syntax::parse;
use syntex_syntax::visit::{walk_crate, FnKind, Visitor};

struct VisitFns {}

impl<'ast> Visitor<'ast> for VisitFns {
    fn visit_fn(&mut self, fk: FnKind<'ast>, fd: &'ast FnDecl, s: Span, _: NodeId) {
        match fk {
            FnKind::ItemFn(ident, _generics, _unsafety, _constness, _abi, _visibility, _block) => {
                println!("visit item fn {:?}", ident)
            }
            FnKind::Method(ident, _sig, _vis, _block) => {
                println!("visit method fn {:?}", ident)
            }
            FnKind::Closure(expr) => {
                println!("visit closure fn {:?}", expr)
            }
            _ => (),
        }
    }
}

fn main() -> Result<()> {
    let mapping = FilePathMapping::empty();
    let session = parse::ParseSess::new(mapping);
    let srcpath = PathBuf::from(&args().nth(1).expect("a Rust source file name"));
    let crat =
        parse::parse_crate_from_file(&srcpath, &session).map_err(|mut diag| {
            diag.emit();
            anyhow!("Failed to parse crate")
        })?;
    // dbg!(&crat);
    walk_crate(&mut VisitFns {}, &crat);
    Ok(())
}

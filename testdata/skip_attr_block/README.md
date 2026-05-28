# skip_attr_block

Exercises `#[mutants::skip]` placed on a block expression `{ ... }` inside a
function body. Each function in `src/lib.rs` annotates a block in a different
syntactic position and pairs it with un-annotated sibling code so that the
expected behaviour is unambiguous:

- `statement_position` — `#[..] { ... }` used as a statement.
- `tail_block` — `#[..] { ... }` used as the function's tail expression.
- `labeled_block` — `#[..] 'lbl: { ... }` used as the function's tail
  expression.
- `unannotated_sibling` — no skip attribute; mutants must still be produced.

Inside each annotated block every mutant cargo-mutants would normally produce
must be suppressed; in `unannotated_sibling`, mutants must be produced as
usual.

## Why `cfg_attr` wrapping?

Stable Rust gates custom (proc-macro) attributes on expressions and block
expressions behind the unstable `stmt_expr_attributes` and
`proc_macro_hygiene` features, so the direct form `#[mutants::skip] { ... }`
does not compile on stable. This tree uses
`#[cfg_attr(any(), mutants::skip)]` instead: the built-in `any()` cfg
predicate with no operands evaluates to false at compile time, so rustc
strips the whole `cfg_attr` and never sees the inner `mutants::skip`
proc-macro attribute. cargo-mutants still recognises it via
`visit::attr_is_mutants_skip` and applies the suppression. Using `any()`
rather than a made-up cfg name avoids the `unexpected_cfgs` lint that
rustc emits for unrecognised cfg flags.

The unit tests under `src/visit/test/skip_attr_expr_block.rs` cover the
direct `#[mutants::skip]` form (which only requires syn parsing).

Because the inner proc-macro is never expanded, this tree does not need
to depend on the `mutants` crate at all.

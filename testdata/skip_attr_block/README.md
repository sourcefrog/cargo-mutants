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
`#[cfg_attr(mutants, mutants::skip)]` instead: the `mutants` cfg is never
set during normal builds or `cargo check --tests`, so rustc strips the
`cfg_attr` to nothing and never sees the inner `mutants::skip` proc-macro
attribute. cargo-mutants still recognises it via
`visit::attr_is_mutants_skip` and applies the suppression. This matches
the convention already used by `testdata/cfg_attr_mutants_skip`.

The unit tests under `src/visit/test/skip_attr_expr_block.rs` cover the
direct `#[mutants::skip]` form (which only requires syn parsing).

Because the inner proc-macro is never expanded, this tree does not need
to depend on the `mutants` crate at all.

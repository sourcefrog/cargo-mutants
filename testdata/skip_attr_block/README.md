# skip_attr_block

Exercises `#[mutants::skip]` placed directly on a block expression
`{ ... }` inside a function body. Each function in `src/lib.rs` annotates
a block in a different syntactic position and pairs it with un-annotated
sibling code so that the expected behavior is unambiguous:

- `statement_position` — `#[mutants::skip] { ... }` used as a statement.
- `tail_block` — `#[mutants::skip] { ... }` used as the function's tail
  expression.
- `labeled_block` — `#[mutants::skip] 'block: { ... }` used as the
  function's tail expression.
- `unannotated_sibling` — no skip attribute; mutants must still be
  produced.

Inside each annotated block every mutant cargo-mutants would normally
produce must be suppressed; in `unannotated_sibling`, mutants must be
produced as usual.

## Nightly-only

Custom proc-macro attributes on expressions and block expressions are
nightly-only (`stmt_expr_attributes` and `proc_macro_hygiene`). The
crate-level `#![cfg_attr(mutants_nightly, feature(...))]` enables those
features only when the `mutants_nightly` cfg is set, and the integration
test that consumes this tree is `#[ignore]`d unless the same cfg is set.

See `AGENTS.md` in the repository root for how to opt in.

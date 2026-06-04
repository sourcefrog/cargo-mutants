# skip_attr_expressions

Exercises `#[mutants::skip]` placed directly on expressions other than
block expressions. Each function in `src/lib.rs` pairs an annotated
expression of a given genre with an un-annotated sibling expression of
the same genre so that the expected behavior is unambiguous:

- `call_expr` — `#[mutants::skip]` on a call expression suppresses
  mutants generated for its argument expressions.
- `method_call_expr` — same, on a method-call expression.
- `match_expr` — on a `match` expression, suppresses both arm-deletion
  and guard-replacement mutants.
- `struct_expr` — on a struct literal that has a `..Default::default()`
  base, suppresses the field-deletion mutants generated for the literal.
- `unary_expr` — on a unary `!` expression, suppresses the unary
  deletion mutant.

Inside each annotated expression every mutant cargo-mutants would
normally produce must be suppressed; in the un-annotated sibling in the
same function, mutants must be produced as usual. Block-expression skip
(`#[mutants::skip] { ... }`) is covered separately by the `skip_attr_block`
tree.

## Nightly-only

Custom proc-macro attributes on expressions and statements are
nightly-only (`stmt_expr_attributes` and `proc_macro_hygiene`). The
crate-level `#![cfg_attr(mutants_nightly, feature(...))]` enables those
features only when the `mutants_nightly` cfg is set, and the integration
test that consumes this tree is `#[ignore]`d unless the same cfg is set.

See `AGENTS.md` in the repository root for how to opt in.

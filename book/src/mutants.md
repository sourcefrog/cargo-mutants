# Generating mutants

cargo mutants generates mutants by inspecting the existing
source code and applying a set of rules to generate new code
that is likely to compile but have different behavior.

Mutants each have a "genre". In the current release, the only mutation genre is
`FnValue`, where a function's body is replaced with a value of the same type.
This checks that the tests:

1. Observe any side effects of the original function.
2. Distinguish return values.

More mutation genres and patterns will be added in future releases.

| Return type | Mutation pattern |
| ----------- | ---------------- |
| `()`        | `()` (return unit, with no side effects) |
| signed integers | 0, 1, -1    |
| unsigned integers | 0, 1      |
| `bool`      | `true`, `false` |
| `String`    | `String::new()`, `"xyzzy".into()` |
| `&'_ str` . | `""`, `"xyzzy"` |
| `&mut ...`  | `Box::leak(Box::new(...))` |
| `Result<T>`    | `Ok(...)`, [and an error if configured](error-values.md) |
| `Option<T>`    | `Some(...)`, `None` |
| (any other) | `Default::default()` (and hope the type implements `Default`) |

Some of these values may not be valid for all types: for example, returning
`Default::default()` will work for many types, but not all. In this case the
mutant is said to be "unviable": by default these are counted but not printed,
although they can be shown with `--unviable`.

cargo-mutants recurses into types like `Result` and `Option`, generating further
mutants. For example, `Result<bool>` can generate `Ok(true)` and `Ok(false)`.
The recursion can nest for types like `Result<Option<String>>`.
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

| Return type       | Mutation pattern |
| ----------------- | ---------------- |
| `()`              | `()` (return unit, with no side effects) |
| signed integers   | `0, 1, -1`    |
| unsigned integers | `0, 1`      |
| floats            | `0.0, 1.0, -1.0`                                        |
| `NonZeroI*`       | `1, -1`     |
| `NonZeroU*`       | `1`         |
| `bool`      | `true`, `false` |
| `String`    | `String::new()`, `"xyzzy".into()` |
| `&'_ str` . | `""`, `"xyzzy"` |
| `&mut ...`  | `Box::leak(Box::new(...))` |
| `Result<T>`    | `Ok(...)` , [and an error if configured](error-values.md) |
| `Option<T>`    | `Some(...)`, `None` |
| `Box<T>`       | `Box::new(...)`                                            |
| `Vec<T>`       | `vec![]`, `vec![...]`                                      |
| `&T`           | `&...` (all replacements for T)                            |
| (any other)    | `Default::default()`                                       |

`...` in the mutation patterns indicates that the type is recursively mutated.
 For example, `Result<bool>` can generate `Ok(true)` and `Ok(false)`.
The recursion can nest for types like `Result<Option<String>>`.

Some of these values may not be valid for all types: for example, returning
`Default::default()` will work for many types, but not all. In this case the
mutant is said to be "unviable": by default these are counted but not printed,
although they can be shown with `--unviable`.

# Generating mutants

cargo mutants generates mutants by inspecting the existing
source code and applying a set of rules to generate new code
that is likely to compile but have different behavior.

In the current release, the only mutation pattern is to
replace function bodies with a value of the same type.
This checks that the tests:

1. Observe any side effects of the original function.
2. Distinguish return values.

More mutation rules will be added in future releases.

| Return type | Mutation pattern |
| ----------- | ---------------- |
| `()`        | `()` (return unit, with no side effects) |
| `bool`      | `true`, `false` |
| `String`    | `String::new()`, `"xyzzy".into()` |
| `&'_ str` . | `""`, `"xyzzy"` |
| `Result`    | `Ok(Default::default())`, [and an error if configured](error-values.md) |
| (any other) | `Default::default()` (and hope the type implements `Default`) |

Some of these values may not be valid for all types: for example, returning
`Default::default()` will work for many types, but not all. In this case the
mutant is said to be "unviable": by default these are counted but not printed,
although they can be shown with `--unviable`.

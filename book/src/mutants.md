# Generating mutants

cargo mutants generates mutants by inspecting the existing
source code and applying a set of rules to generate new code
that is likely to compile but have different behavior.

Mutants each have a "genre", each of which is described below.

## Functions that are excluded from mutation

Some functions are automatically excluded from mutation:

- Functions marked with `#[cfg(test)]` or in files marked with `#![cfg(test)]`
- Test functions: functions with attributes whose path ends with `test`, including `#[test]`, `#[tokio::test]`, `#[sqlx::test]`, and similar testing framework attributes
- Functions marked with `#[mutants::skip]`
- `unsafe` functions

You can also explicitly [skip functions](skip.md) or [filter which functions are mutated](filter_mutants.md).

## Replace function body with value

The `FnValue` genre of mutants replaces a function's body with a value that is guessed to be of the right type.

This checks that the tests:

1. Observe any side effects of the original function.
2. Distinguish return values.

More mutation genres and patterns will be added in future releases.

| Return type       | Mutation pattern |
| ----------------- | ---------------- |
| `()`              | `()` (return unit, with no side effects) |
| signed integers   | `0, 1, -1`    |
| unsigned integers | `0, 1`      |
| floats            | `0.0, 1.0, -1.0`                                           |
| `NonZeroI*`       | `1.try_into().unwrap(), (-1).try_into().unwrap()`            |
| `NonZeroU*`       | `1.try_into().unwrap()`                                    |
| `bool`            | `true`, `false` |
| `String`          | `String::new()`, `"xyzzy".into()` |
| `&'_ str` .       | `""`, `"xyzzy"` |
| `&T`              | `Box::leak(Box::new(...))` |
| `&mut T`          | `Box::leak(Box::new(...))` |
| `&[T]`            | `Vec::leak(...)` |
| `&mut [T]`            | `Vec::leak(...)` |
| `Result<T>`       | `Ok(...)` , [and an error if configured](error-values.md)  |
| `Option<T>`       | `Some(...)`, `None` |
| `Box<T>`          | `Box::new(...)`                                            |
| `Vec<T>`          | `vec![]`, `vec![...]`                                      |
| `Arc<T>`          | `Arc::new(...)`                                            |
| `Rc<T>`           | `Rc::new(...)`                                             |
| `BinaryHeap`, `BTreeSet`, `HashSet`, `LinkedList`, `VecDeque` | empty and one-element collections |
| `BTreeMap`, `HashMap` | empty map and the product of all key and value replacements |
| `Cow<'_, T>`      | `Cow::Borrowed(t)`, `Cow::Owned(t.to_owned())`             |
| `[T; L]`          | `[r; L]` for all replacements of T                         |
| `&[T]`, `&mut [T]`| Leaked empty and one-element vecs                          |
| `&T`              | `&...` (all replacements for T)                            |
| `HttpResponse`    | `HttpResponse::Ok().finish`                                |
| `(A, B, ...)`     | `(a, b, ...)` for the product of all replacements of A, B, ... |
| `impl Iterator`   | Empty and one-element iterators of the inner type           |
| (any other)       | `Default::default()`                                       |

`...` in the mutation patterns indicates that the type is recursively mutated.
 For example, `Result<bool>` can generate `Ok(true)` and `Ok(false)`.
The recursion can nest for types like `Result<Option<String>>`.

Some of these values may not be valid for all types: for example, returning
`Default::default()` will work for many types, but not all. In this case the
mutant is said to be "unviable": by default these are counted but not printed,
although they can be shown with `--unviable`.

## Binary operators

Binary operators are replaced with other binary operators in expressions
like `a == 0`.

| Operator | Replacements       |
| -------- | ------------------ |
| `==`     | `!=`               |
| `!=`     | `==`               |
| `&&`     | `\|\|`             |
| `\|\|`   | `&&`,              |
| `<`      | `==`, `>`          |
| `>`      | `==`, `<`          |
| `<=`     | `>`                |
| `>=`     | `<`                |
| `+`      | `-`, `*`           |
| `-`      | `+`, `/`           |
| `*`      | `+`, `/`           |
| `/`      | `%`, `*`           |
| `%`      | `/`, `+`           |
| `<<`     | `>>`               |
| `>>`     | `<<`               |
| `&`      | `\|`,`^`           |
| `\|`     | `&`, `^`           |
| `^`      | `&`, `\|`          |
| `&=`     | `\|=`              |
| `\|=`    | `&=`               |
| `^=`     | `\|=`, `&=`        |
| `+=`, `-=`, `*=`, `/=`, `%=`, `<<=`, `>>=` | assignment corresponding to the operator above |

Equality operators are not currently replaced with comparisons like `<` or `<=`
because they are
too prone to generate false positives, for example when unsigned integers are compared to 0.

The bitwise assignment operators `&=` and `|=` are not mutated to `^=` because in code that accumulates bits (e.g., `bitmap |= new_bits`), `|=` and `^=` produce the same result when starting from zero, making such mutations uninformative.

## Unary operators

Unary operators are deleted in expressions like `-a` and `!a`.
They are not currently replaced with other unary operators because they are too prone to
generate unviable cases (e.g. `!1.0`, `-false`).

## Match arms

Entire match arms are deleted in match expressions when a wildcard pattern is present in one of the arms.
Match expressions without a wildcard pattern would be too prone to unviable mutations of this kind.

## Match arm guards

Match arm guard expressions are replaced with `true` and `false`.

## Struct literal fields

Individual fields are deleted from struct literals that have a base (default) expression,
such as `..Default::default()` or `..base_value`.

For example, in this code:

```rust
let cat = Cat {
    name: "Felix",
    coat: Coat::Tuxedo,
    ..Default::default()
};
```

cargo-mutants will generate two mutants: one deleting the `name` field and one deleting
the `coat` field. This checks that tests verify that each field is set correctly and not
just relying on the default values.

Struct literals without a base expression are not mutated in this way, because deleting
a required field would make the code fail to compile.

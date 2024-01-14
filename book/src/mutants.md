# Generating mutants

cargo mutants generates mutants by inspecting the existing
source code and applying a set of rules to generate new code
that is likely to compile but have different behavior.

Mutants each have a "genre", each of which is described below.

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
| floats            | `0.0, 1.0, -1.0`                                        |
| `NonZeroI*`       | `1, -1`     |
| `NonZeroU*`       | `1`         |
| `bool`            | `true`, `false` |
| `String`          | `String::new()`, `"xyzzy".into()` |
| `&'_ str` .       | `""`, `"xyzzy"` |
| `&mut ...`        | `Box::leak(Box::new(...))` |
| `Result<T>`       | `Ok(...)` , [and an error if configured](error-values.md) |
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
| `+=` and similar assignments | assignment corresponding to the line above |

Equality operators are not currently replaced with comparisons like `<` or `<=`
because they are
too prone to generate false positives, for example when unsigned integers are compared to 0.

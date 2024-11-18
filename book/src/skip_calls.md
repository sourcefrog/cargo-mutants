# Skipping function calls

Using the `--skip-calls` argument and config key you can tell cargo-mutants not to mutate the arguments to calls to specific named functions and methods.

For example:

```sh
cargo mutants --skip-calls=skip_this,and_this
```

or in `.cargo/mutants.toml`

```toml
skip_calls = ["skip_this", "and_this"]
```

The command line arguments are added to the values specified in the configuration.

The names given in the option and argument are matched against the final component of the path in each call, disregarding any type parameters. For example, the default value of `with_capacity` will match `std::vec::Vec::<String>::with_capacity(10)`.

This is separate from [skipping mutation of the body of a function](attrs.md), and only affects the generation of mutants within the call expression, typically in its arguments.

By default, calls to functions called `with_capacity` are not mutated. The defaults can be turned off using `--skip-calls-defaults=false`.

## `with_capacity`

The motivating example for this feature is Rust's `with_capacity` function on `Vec` and other collections, which preallocates capacity for a slight performance gain.

```rust
    let mut v = Vec::with_capacity(4 * n);
```

cargo-mutants normally mutates expressions in function calls, and in this case it will try mutating the capacity expression to `4 / n` etc.

These mutations would change the program behavior. Assuming the original calculation is correct the mutation then the mutation will likly be wrong.

However, many authors may feel that preallocating the estimated memory needs is worth doing but not worth specifically writing tests or assertions for, and so they would like to skip generating mutants in any calls to these functions.

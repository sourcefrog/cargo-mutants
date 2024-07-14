# Strict lints

Because cargo-mutants builds versions of your tree with many heuristically injected errors, it may not work well in trees that are configured to treat warnings as errors.

For example, mutants that delete code are likely to cause some parameters to be seen as unused, which will cause problems with trees that configure `#[deny(unused)]`. This will manifest as an excessive number of mutants being reported as "unviable".

There are a few possible solutions:

1. Define a feature flag for mutation testing, and use `cfg_attr` to enable strict warnings only when not testing mutants.
2. Use the `cargo mutants --cap-lints=true` command line option, or the `cap_lints = true` config option.

`--cap_lints=true` also disables rustc's detection of long-running const expression evaluation, so may cause some builds to fail. If that happens in your tree, you can set a [build timeout](timeouts.md).

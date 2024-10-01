# Mutating code using macros

cargo-mutants will mutate the contents of `#[proc_macro]` functions defined in the current crate, and run tests to see if those mutations are caught.

cargo-mutants does not currently mutate calls to macros, or the expansion of a macro, or the definition of declarative `macro_rules` macros. As a result on code that is mostly produced by macro expansion it may not find many mutation opportunities.

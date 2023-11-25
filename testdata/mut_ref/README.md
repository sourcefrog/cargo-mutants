# `mut_ref` test case

An example of a function that returns a mut reference into a mut argument.

cargo-mutants generates a mutant that changes the return value to a mut reference
into a new value on the heap. As a result, tests that check the return value
should fail.

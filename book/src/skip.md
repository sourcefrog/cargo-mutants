# Skipping untestable code

Some functions may be inherently hard to cover with tests, for example if:

* Generated mutants cause tests to hang.
* You've chosen to test the functionality by human inspection or some higher-level integration tests.
* The function has side effects or performance characteristics that are hard to test.
* You've decided the function is not important to test.

There are three ways to skip mutating some code:

1. [Marking the function with an attribute](attrs.md) within the source file.
2. [Filtering by path](skip_files.md) in the config file or command line.
3. [Filtering by function and mutant name](filter_mutants.md) in the config file or command line.

The results of all these filters can be previewed using the `--list` option.

## Which filtering method to use?

* If some particular functions are hard to test with cargo-mutants, use an attribute, so that the skip is visible in the code.
* If a whole module is untestable, use a filter by path in the config file, so that the filter's stored in the source tree and covers any new code in that module.
* If you want to permanently ignore a class of functions, such as `Debug` implementations, use a regex filter in the config file.
* If you want to run cargo-mutants just once, focusing on a subset of files, functions, or mutants, use command line options to filter by name or path.

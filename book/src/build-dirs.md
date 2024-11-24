# Build directories

cargo-mutants builds mutated code in a temporary directory, containing a copy of your source tree with each mutant successively applied. With `--jobs`, multiple build directories are used in parallel.

## Build-in ignores

Files or directories matching these patterns are not copied:

    .git
    .hg
    .bzr
    .svn
    _darcs
    .pijul

## gitignore

From 23.11.2, by default, cargo-mutants will not copy files that are excluded by gitignore patterns, to make copying faster in large trees.

gitignore filtering is only used within trees containing a `.git` directory.

The filter, based on the [`ignore` crate](https://docs.rs/ignore/), also respects global git ignore configuration in the home directory, as well as `.gitignore` files within the tree.

This behavior can be turned off with `--gitignore=false`, causing ignored files to be copied.

# Copying the tree

By default, cargo-mutants copies your tree to a temporary directory before mutating and building it. This behavior is turned of by the [`--in-place`](in-place.md) option, which builds mutated code in the original source directory.

When the [`--jobs`](parallelism.md) option is used, one build directory is created per job.

Some filters are applied while copying the tree, which can be configured by options.

## Troubleshooting tree copies

If the baseline tests fail in the copied directory it is a good first debugging step to try building with `--in-place`.

## `.git` and other version control directories

By default, files or directories matching these patterns are not copied, because they can be large and typically are not needed to build the source:

    .git
    .hg
    .jj
    .bzr
    .svn
    _darcs
    .pijul

If your tree's build or tests require the VCS directory then it can be copied with `--copy-vcs=true` or by setting `copy_vcs = true` in `.cargo/mutants.toml`.

## `.gitignore`

From 25.0.2, by default, cargo-mutants will copy all files except those explicitly excluded (such as `target/`, `mutants.out`, and VCS directories). Previously, gitignore patterns were respected by default.

gitignore filtering is only used within trees containing a `.git` directory.

The filter, based on the [`ignore` crate](https://docs.rs/ignore/), also respects global git ignore configuration in the home directory, as well as `.gitignore` files within the tree.

gitignore filtering can be enabled with `--gitignore=true`, causing files matching gitignore patterns to be excluded from copying.

The `target/` directory is excluded by default, regardless of gitignore settings, to avoid copying large build artifacts that are typically not needed for mutation testing. This can be overridden with `--copy-target=true` if your tests depend on existing build artifacts, or by setting `copy_target = true` in `.cargo/mutants.toml`.

Note that if you set `--gitignore=true` and `--copy-target=true` and your `target/` is excluded by gitignore files (which is common) then it will not be copied.

## `mutants.out`

`mutants.out` and `mutants.out.old` are never copied, even if they're not covered by `.gitignore`.

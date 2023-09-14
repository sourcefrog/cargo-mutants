# Workspace and package support

cargo-mutants supports testing Cargo workspaces that contain multiple packages. The entire workspace tree is copied.

By default, all source files in all packages in the workspace are tested.

**NOTE: This behavior might not be the best choice, and this may change in future.**

You can use the `--file` options to restrict cargo-mutants to testing only files
from some subdirectory, e.g. with `-f "utils/**/*.rs"`. (Remember to quote globs
on the command line, so that the shell doesn't expand them.) You can use `--list` or
`--list-files` to preview the effect of filters.

For each mutant, only the containing package's tests are run, on the theory that
each package's tests are responsible for testing the package's code.

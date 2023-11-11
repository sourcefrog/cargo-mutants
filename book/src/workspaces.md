# Workspace and package support

cargo-mutants supports testing Cargo workspaces that contain multiple packages. The entire workspace tree is copied.

By default, cargo-mutants has [the same behavior as Cargo](https://doc.rust-lang.org/cargo/reference/workspaces.html):

* If `--workspace` is given, all packages in the workspace are tested.
* If `--package` is given, the named packages are tested.
* If the starting directory (or `-d` directory) is in a package, that package is tested.
* Otherwise, the starting directory must be in a virtual workspace. If it specifies default members, they are tested. Otherwise, all packages are tested.

For each mutant, only the containing package's tests are run, on the theory that
each package's tests are responsible for testing the package's code.

The baseline tests exercise all and only the packages for which mutants will
be generated.

You can also use the `--file` options to restrict cargo-mutants to testing only files
from some subdirectory, e.g. with `-f "utils/**/*.rs"`. (Remember to quote globs
on the command line, so that the shell doesn't expand them.) You can use `--list` or
`--list-files` to preview the effect of filters.

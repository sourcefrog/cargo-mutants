# Workspace and package support

cargo-mutants now supports testing Cargo workspaces that contain multiple packages.

All source files in all packages in the workspace are tested. For each mutant, only the containing package's tests are run.

TODO: An example?


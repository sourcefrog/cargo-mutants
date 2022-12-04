# Workspace and package support

cargo-mutants supports testing Cargo workspaces that contain multiple packages. The entire workspace tree is copied.

All source files in all packages in the workspace are tested.

For each mutant, only the containing package's tests are run, on the theory that each package's tests are responsible for testing the package's code.

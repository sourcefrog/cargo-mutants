# Incremental tests of pull requests

You can use `--in-diff` to test only the code that has changed in a pull request. This can be useful for incremental testing in CI, where you want to test only the code that has changed since the last commit.

For example, you can use the following workflow to test only the code that has changed in a pull request:

```yaml
name: Tests

permissions:
  contents: read

env:
  CARGO_TERM_COLOR: always

on:
  push:
    branches:
      - main
  pull_request:

jobs:
  incremental-mutants:
    runs-on: ubuntu-latest
    if: github.event_name == 'pull_request'
    steps:
      - uses: actions/checkout@v3
        with:
          fetch-depth: 0
      - name: Relative diff
        run: |
          git branch -av
          git diff origin/${{ github.base_ref }}.. | tee git.diff
      - uses: Swatinem/rust-cache@v2
      - uses: taiki-e/install-action@v2
        name: Install cargo-mutants using install-action
        with:
          tool: cargo-mutants
      - name: Mutants
        run: |
          cargo mutants --no-shuffle -j 2 -vV --in-diff git.diff
      - name: Archive mutants.out
        uses: actions/upload-artifact@v3
        if: always()
        with:
          name: mutants-incremental.out
          path: mutants.out
```

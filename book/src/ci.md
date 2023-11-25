# Continuous integration

Here is an example of a GitHub Actions workflow that runs mutation tests and uploads the results as an artifact. This will fail if it finds any uncaught mutants.

```yml
name: cargo-mutants

on: [pull_request, push]

jobs:
  cargo-mutants:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Install cargo-mutants
        run: cargo install --locked cargo-mutants
      - name: Run mutant tests
        run: cargo mutants -- --all-features
      - name: Archive results
        uses: actions/upload-artifact@v3
        if: failure()
        with:
          name: mutation-report
          path: mutants.out
```

The workflow used by cargo-mutants on itself can be seen at
<https://github.com/sourcefrog/cargo-mutants/blob/main/.github/workflows/mutate-self.yaml>.

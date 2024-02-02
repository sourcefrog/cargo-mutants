# Incremental tests of pull requests

You can use `--in-diff` to test only the code that has changed in a pull request. This can be useful for incremental testing in CI, where you want to test only the code that has changed since the last commit.

For example, you can use the following workflow to test only the code that has changed in a pull request:

```yaml
{{#include ../../examples/workflows/in-diff.yml}}
```

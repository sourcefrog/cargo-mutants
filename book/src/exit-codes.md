# Exit codes

cargo-mutants returns an exit code that can be used by scripts or CI.

* **0**: Success! Every viable mutant that was tested was caught by a test.

* **1**: Usage error: bad command-line arguments etc.

* **2**: Found some mutants that were not covered by tests.

* **3**: Some tests timed out: possibly the mutations caused an infinite loop,
  or the timeout is too low.

* **4**: The baseline tests are already failing or hanging before any mutations are
  applied, so no mutations were tested.

For more detailed machine-readable information, use the [`mutants.out` directory](mutants-out.md).

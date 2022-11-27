# Exit codes

* **0**: Success. No mutants were found that weren't caught by tests.

* **1**: Usage error: bad command-line arguments etc.

* **2**: Found some mutants that were not covered by tests.

* **3**: Some tests timed out: possibly the mutatations caused an infinite loop,
  or the timeout is too low.

* **4**: The tests are already failing or hanging before any mutations are
  applied, so no mutations were tested.

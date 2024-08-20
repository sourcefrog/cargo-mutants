# Iterating on missed mutants

When you're working to improve test coverage in a tree, you might use a process like this:

1. Run `cargo-mutants` to find code that's untested, possibly filtering to some selected files.

2. Think about why some mutants are missed, and then write tests that will catch them.

3. Run cargo-mutants again to learn whether your tests caught all the mutants, or if any remain.

4. Repeat until everything is caught.

You can speed up this process by using the `--iterate` option. This tells cargo-mutants to skip mutants that were either caught or unviable in a previous run, and to accumulate the results.

You can run repeatedly with `--iterate`, adding tests each time, until all the missed mutants are caught (or skipped.)

## How it works

When `--iterate` is given, cargo-mutants reads `mutants.out/caught.txt`, `previously_caught.txt`, and `unviable.txt` before renaming that directory to `mutants.out.old`. If those files don't exist, the lists are assumed to be empty.

Mutants are then tested as usual, but excluding all the mutants named in those files. `--list --iterate` also applies this exclusion and shows you which mutants will be tested.

Mutants are matched based on their file name, line, column, and description, just as shown in `--list` and in those files. As a result, if you insert or move text in a source file, some mutants may be re-tested.

After testing, all the previously caught, caught, and unviable are written into `previously_caught.txt` so that they'll be excluded on future runs.

`previously_caught.txt` is only written when `--iterate` is given.

## Caution

`--iterate` is a heuristic, and makes the assumption that any new changes you make won't reduce coverage, which might not be true. After you think you've caught all the mutants, you should run again without `--iterate` to make sure.

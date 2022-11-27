# Getting started

Just run `cargo mutants` in a Rust source directory, and it will point out
functions that may be inadequately tested:

```none
; cargo mutants
Found 14 mutants to test
Copy source to scratch directory ... 0 MB in 0.0s
Unmutated baseline ... ok in 1.6s build + 0.3s test
Auto-set test timeout to 20.0s
src/lib.rs:386: replace <impl Error for Error>::source -> Option<&(dyn std::error::Error + 'static)> with Default::default() ... NOT CAUGHT in 0.6s build + 0.3s test
src/lib.rs:485: replace copy_symlink -> Result<()> with Ok(Default::default()) ... NOT CAUGHT in 0.5s build + 0.3s test
14 mutants tested in 0:08: 2 missed, 9 caught, 3 unviable
```

In v0.5.1 of the `cp_r` crate, the `copy_symlink` function was reached by a test
but not adequately tested.

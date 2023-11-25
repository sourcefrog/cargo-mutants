/// A module that's not only for tests, but should be excluded anyhow.
#[mutants::skip]
mod skip_this_mod {
    fn inside_skipped_mod() {}
}

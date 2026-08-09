# `exclude_re` on external modules

Checks that `#[mutants::exclude_re]` scopes are inherited by external module
files and further external modules, while inline modules and file inner
attributes continue to behave the same way.

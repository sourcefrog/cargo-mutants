#![mutants::exclude_re("replace inner_f")]

pub fn inner_f() -> bool {
    true
}

pub fn inner_g() -> bool {
    true
}

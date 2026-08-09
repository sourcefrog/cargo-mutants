#[mutants::exclude_re("replace f")]
#[mutants::exclude_re("nested_f")]
mod external;

#[mutants::exclude_re("inline_f")]
mod inline {
    pub fn inline_f() -> bool {
        true
    }

    pub fn inline_g() -> bool {
        true
    }
}

mod inner_attr;

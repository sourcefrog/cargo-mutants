//! These functions have empty bodies; we cannot usefully mutate them.

fn just_empty() {}

fn only_a_comment() { /* it's still basically empty */
}

struct Foo();

impl Foo {
    fn empty_in_foo(&self) { /* also caught */
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn empty_fns_do_nothing() {
        just_empty();
        only_a_comment();
        Foo().empty_in_foo();
    }
}

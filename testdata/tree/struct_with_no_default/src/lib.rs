//! Example of a struct with no Default that generates unviable mutants.

#![allow(dead_code)]

pub struct S {
    a: &'static str,
    b: usize,
}

// This can't be called "new" because that name is specifically excluded.
pub fn make_an_s() -> S {
    S {
        a: "on the beach",
        b: 99,
    }
}

#[test]
fn test_new_s() {
    let s = make_an_s();
    assert!(!s.a.is_empty());
    assert_eq!(s.b, 99);
}

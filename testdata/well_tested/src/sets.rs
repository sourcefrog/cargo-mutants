use std::collections::BTreeSet;

fn make_a_set() -> BTreeSet<String> {
    let mut s = BTreeSet::new();
    s.insert("one".into());
    s.insert("two".into());
    s
}

#[test]
fn set_has_two_elements() {
    assert_eq!(make_a_set().len(), 2);
}

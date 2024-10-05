use cargo_mutants_testdata_proc_macro::static_len;

#[test]
fn static_len() {
    assert_eq!(static_len!(2, 3, 4, 5), 4);
}

#[test]
fn static_len_empty() {
    assert_eq!(static_len!(), 0);
}

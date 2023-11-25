use cargo_mutants_testdata_insta::say_hello;

#[test]
fn say_hello_vs_insta_snapshot() {
    let name = "Robin";
    insta::assert_snapshot!(say_hello(name));
}

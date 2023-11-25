#![feature(box_patterns)]
fn box_an_int() -> Box<i32> {
    Box::new(5)
}

#[test]
fn unbox_by_pattern() {
    let box a = box_an_int();
    assert_eq!(a, 5);
}

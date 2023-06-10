use std::sync::Arc;

fn return_arc() -> Arc<String> {
    Arc::new(String::from("hello!"))
}

#[test]
fn returns_hello() {
    assert_eq!(return_arc().as_ref(), "hello!");
}

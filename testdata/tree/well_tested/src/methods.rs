struct Foo {
    i: u32,
}

impl Foo {
    pub fn new() -> Foo {
        Foo { i: 32 }
    }

    pub fn double(&mut self) {
        self.i *= 2;
    }
}

#[mutants::skip]
impl Default for Foo {
    fn default() -> Self {
        Foo { i: 1 }
    }
}

#[test]
fn double() {
    let mut foo = Foo::new();
    assert_eq!(foo.i, 32);
    foo.double();
    assert_eq!(foo.i, 64);
    foo.double();
    assert_eq!(foo.i, 128);
}

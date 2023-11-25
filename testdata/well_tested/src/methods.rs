//! Demonstrate mutation of impl methods.

#![allow(clippy::disallowed_names)] // "Foo" is just an example name.

use std::fmt;

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

impl fmt::Display for Foo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Foo {}", self.i)
    }
}

impl fmt::Debug for &Foo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "&Foo {}", self.i)
    }
}

impl Default for Foo {
    fn default() -> Self {
        Foo::new()
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

#[test]
fn default() {
    let foo = Foo::default();
    assert_eq!(foo.i, 32);
}

#[test]
fn new_foo() {
    let foo = Foo::new();
    assert_eq!(foo.i, 32);
}

#[test]
fn display_foo() {
    assert_eq!(format!("{}", Foo { i: 123 }), "Foo 123");
}

#[test]
fn debug_ref_foo() {
    assert_eq!(format!("{:?}", &Foo { i: 123 }), "&Foo 123");
}

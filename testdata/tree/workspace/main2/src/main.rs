use utils::a_public_module::*;
use std::fmt;

fn main() {
    println!("Print works from main 2 binary");

    print_from_utils();
}
struct Bar {
    i: u32,
}

impl Bar {
    pub fn new() -> Bar {
        Bar { i: 32 }
    }

    pub fn double(&mut self) {
        self.i *= 2;
    }
}

impl fmt::Display for Bar {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Foo {}", self.i)
    }
}

impl fmt::Debug for &Bar {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "&Foo {}", self.i)
    }
}

impl Default for Bar {
    fn default() -> Self {
        Bar::new()
    }
}

#[test]
fn test1() {
    assert!(true);
}

#[test]
fn test2() {
    assert!(true);
}
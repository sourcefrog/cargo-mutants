use utils::a_public_module::*;
use std::fmt;

fn main() {
    for i in 1..=6 {
        println!("{}! = {}", i, factorial(i));
    }
}

fn factorial(n: u32) -> u32 {
    let mut a = 1;
    for i in 2..=n {
        a *= i;
    }
    a
}

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
fn test1() {
    assert!(true);
}
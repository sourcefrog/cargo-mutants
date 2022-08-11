//! Demonstrate mutation of impl methods.

#![allow(clippy::blacklisted_name)] // "Foo" is just an example name.

use std::fmt;

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
//! Everything is skipped, so this tests the case where no mutants are found.

#[mutants::skip]
fn main() {
    for i in 1..=6 {
        println!("{}! = {}", i, factorial(i));
    }
}

#[mutants::skip]
fn factorial(n: u32) -> u32 {
    let mut a = 1;
    for i in 2..=n {
        a *= i;
    }
    a
}

#[test]
fn test_factorial() {
    println!("factorial({}) = {}", 6, factorial(6)); // This line is here so we can see it in --nocapture
    assert_eq!(factorial(6), 720);
}

struct Thing(u32);

impl From<u32> for Thing {
    #[mutants::skip]
    fn from(x: u32) -> Self {
        Thing(x)
    }
}

impl Thing {
    #[mutants::skip]
    fn value(&self) -> u32 {
        self.0
    }

    // impl fns called "new" are implicitly skipped, because it seems unlikely that
    // we can create the type without them, and it might cause infinite recursions
    // between `default` and `new`.
    fn new() -> Thing {
        Thing(42)
    }

    fn nothing() {
        // Has an empty body (and therefore returns unit), so it's skipped
    }
}

trait Pet {
    #[mutants::skip]
    fn name(&self) -> String {
        "Pixel".to_string()
    }

    fn new() -> Self {
        // This is skipped because it's an impl fn called "new"
        unimplemented!()
    }
}

//! Mutants can be skipped with `cfg_attr` attributes.
//!
//! (We don't currently examine the attributes, we just look for anything mentioning
//! `mutants::skip`.)

#[cfg_attr(test, mutants::skip)]
pub fn factorial(n: u32) -> u32 {
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

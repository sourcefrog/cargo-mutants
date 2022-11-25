#[mutants::skip]
fn main() {
    for i in 1..=6 {
        println!("{}! = {}", i, factorial(i));
    }
}

#[cfg(feature = "needed")]
fn factorial(n: u32) -> u32 {
    let mut a = 1;
    for i in 2..=n {
        a *= i;
    }
    a
}

#[cfg(not(feature = "needed"))]
#[mutants::skip]
fn factorial(_n: u32) -> u32 {
    panic!("needed feature is not enabled");
}

#[test]
fn test_factorial() {
    println!("factorial({}) = {}", 6, factorial(6)); // This line is here so we can see it in --nocapture
    assert_eq!(factorial(6), 720);
}

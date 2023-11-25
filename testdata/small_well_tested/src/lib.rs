//! A small tree with one function with good coverage: a fast-to-run successful
//! case for cargo-mutants.

pub fn factorial(n: u32) -> u32 {
    let mut a = 1;
    for i in 2..=n {
        a *= i;
    }
    a
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_factorial() {
        println!("factorial({}) = {}", 6, factorial(6)); // This line is here so we can see it in --nocapture
        assert_eq!(factorial(6), 720);
    }
}

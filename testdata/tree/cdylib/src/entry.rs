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
        println!("factorial({}) = {}", 6, factorial(6));
        assert_eq!(factorial(6), 720);
    }
}

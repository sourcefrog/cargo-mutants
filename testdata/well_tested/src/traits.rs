//! Test mutation of a default fn in a trait.

trait Something {
    fn is_three(&self, a: usize) -> bool {
        a == 3
    }
}

#[cfg(test)]
mod test {
    use super::*;

    struct Three;

    impl Something for Three {}

    #[test]
    fn test_is_three() {
        assert!(Three.is_three(3));
        assert!(!Three.is_three(4));
    }
}

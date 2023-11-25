fn double_float(a: f32) -> f32 {
    2.0 * a
}

fn is_double(a: u32, b: u32) -> bool {
    a == 2 * b
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn double_zero() {
        assert_eq!(double_float(0.0), 0.0);
    }

    #[test]
    fn double_three() {
        assert_eq!(double_float(3.0), 6.0);
    }

    #[test]
    fn is_double_zero() {
        assert!(is_double(0, 0));
    }

    #[test]
    fn is_double_one() {
        assert!(is_double(2, 1));
        assert!(!is_double(1, 1));
        assert!(!is_double(5, 1));
    }
}

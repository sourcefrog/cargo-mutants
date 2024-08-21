fn double_float(a: f32) -> f32 {
    2.0 * a
}

fn is_double(a: u32, b: u32) -> bool {
    a == 2 * b
}

fn negate_i32(a: i32) -> i32 {
    -a
}

fn negate_f32(a: f32) -> f32 {
    -a
}

fn bitwise_not_i32(a: i32) -> i32 {
    !a
}

fn bitwise_not_u32(a: u32) -> u32 {
    !a
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

    #[test]
    fn negate_one() {
        assert_eq!(negate_i32(1), -1);
        assert_eq!(negate_f32(1.0), -1.0);
    }

    #[test]
    fn negate_two() {
        assert_eq!(negate_i32(2), -2);
        assert_eq!(negate_f32(2.0), -2.0);
    }

    #[test]
    fn bitwise_not_one() {
        assert_eq!(bitwise_not_i32(1), -2);
        assert_eq!(bitwise_not_u32(1), u32::MAX - 1);
    }

    #[test]
    fn bitwise_not_two() {
        assert_eq!(bitwise_not_i32(2), -3);
        assert_eq!(bitwise_not_u32(2), u32::MAX - 2);
    }
}

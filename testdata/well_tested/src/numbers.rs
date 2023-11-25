fn double_float(a: f32) -> f32 {
    2.0 * a
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
}

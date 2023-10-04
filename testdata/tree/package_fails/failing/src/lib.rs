pub fn triple(a: usize) -> usize {
    3 * a
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn triple_3_is_10() {
        assert_eq!(triple(3), 10);
    }
}

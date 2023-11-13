pub fn one() -> String {
    "one".to_owned()
}

#[cfg(test)]
mod test_super {
    use super::*;

    #[test]
    fn test_one() {
        assert_eq!(one(), "one");
    }
}

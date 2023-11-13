pub fn one() -> String {
    "one".to_owned()
}

pub fn two() -> String {
    format!("{}", 2)
}

#[cfg(test)]
mod test_super {
    use super::*;

    #[test]
    fn test_one() {
        assert_eq!(one(), "one");
    }

    #[test]
    fn test_two() {
        assert_eq!(two(), "2");
    }
}

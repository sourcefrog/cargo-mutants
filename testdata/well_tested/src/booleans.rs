fn and(a: bool, b: bool) -> bool {
    a && b
}

fn or(a: bool, b: bool) -> bool {
    a || b
}

fn xor(a: bool, b: bool) -> bool {
    a ^ b
}

fn not(a: bool) -> bool {
    !a
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn all_and() {
        assert_eq!(and(false, false), false);
        assert_eq!(and(true, false), false);
        assert_eq!(and(false, true), false);
        assert_eq!(and(true, true), true);
    }

    #[test]
    fn all_or() {
        assert_eq!(or(false, false), false);
        assert_eq!(or(true, false), true);
        assert_eq!(or(false, true), true);
        assert_eq!(or(true, true), true);
    }

    #[test]
    fn all_xor() {
        assert_eq!(xor(false, false), false);
        assert_eq!(xor(true, false), true);
        assert_eq!(xor(false, true), true);
        assert_eq!(xor(true, true), false);
    }

    #[test]
    fn all_not() {
        assert_eq!(not(false), true);
        assert_eq!(not(true), false);
    }
}

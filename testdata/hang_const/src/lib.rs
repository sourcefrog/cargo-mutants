const fn should_stop_const() -> bool {
    true
}

/// If `should_stop_const` is mutated to return false, then this const block
/// will hang and block compilation.
pub const VAL: i32 = loop {
    if should_stop_const() {
        break 1;
    }
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_const() {
        assert_eq!(VAL, 1);
    }
}

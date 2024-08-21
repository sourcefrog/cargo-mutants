use std::result::Result;

pub fn zero_is_ok(n: u32) -> Result<u32, &'static str> {
    if n == 0 {
        Ok(n)
    } else {
        Err("not zero")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    // These would be better tests but for the sake of the test let's
    // assume nobody wrote them yet.

    // #[test]
    // fn test_even_is_ok() {
    //     assert_eq!(even_is_ok(2), Ok(2));
    //     assert_eq!(even_is_ok(3), Err("number is odd"));
    // }

    // #[test]
    // fn test_even_with_unwrap() {
    //     assert_eq!(even_is_ok(2).unwrap(), 2);
    // }

    #[test]
    fn bad_test_ignores_error_results() {
        // A bit contrived but does the job: never checks that
        // the code passes on values that it should accept.
        assert!(zero_is_ok(1).is_err());
        assert!(zero_is_ok(3).is_err());
    }
}

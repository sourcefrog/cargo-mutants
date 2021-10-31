// /// Function returning a Result.
// fn io_result() -> std::io::Result<

/// Simple easily-recognizable Result.
fn simple_result() -> Result<&'static str, ()> {
    Ok("success")
}

fn error_if_negative(a: i32) -> Result<(), ()> {
    if a < 0 {
        Err(())
    } else {
        Ok(())
    }
}

mod test {
    use super::*;

    #[test]
    fn simple_result_success() {
        assert_eq!(simple_result(), Ok("success"));
    }

    #[test]
    fn error_if_negative() {
        use super::error_if_negative;

        assert_eq!(error_if_negative(0), Ok(()));
        assert_eq!(error_if_negative(-1), Err(()));
        assert_eq!(error_if_negative(1), Ok(()));
    }
}

//! Test tree for `#[mutants::exclude_re("...")]` attribute.
//!
//! This tests that specific mutations can be excluded by regex while
//! keeping other mutations active on the same function.

/// This function has an exclude_re that filters out the "replace with ()" mutation
/// but keeps binary operator mutations.
#[mutants::exclude_re("replace .* with ()")]
pub fn add_numbers(a: i32, b: i32) -> i32 {
    a + b
}

/// This function has no exclude_re, so all mutations should be generated.
pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}

/// This function uses cfg_attr form of exclude_re.
#[cfg_attr(test, mutants::exclude_re("replace .* with"))]
pub fn subtract(a: i32, b: i32) -> i32 {
    a - b
}

/// This function has an exclude_re that filters out binary operator mutations.
#[mutants::exclude_re("replace [+] with")]
pub fn add_one(a: i32) -> i32 {
    a + 1
}

pub struct Calculator;

/// exclude_re on an impl block applies to all methods inside.
#[mutants::exclude_re("replace .* -> bool")]
impl Calculator {
    pub fn is_positive(x: i32) -> bool {
        x > 0
    }

    pub fn double(x: i32) -> i32 {
        x + x
    }
}

/// exclude_re on a trait block applies to all default method implementations.
#[mutants::exclude_re("replace .* -> bool")]
pub trait Checker {
    fn is_valid(&self) -> bool {
        true
    }

    fn score(&self) -> i32 {
        1 + 2
    }
}

/// exclude_re on a mod block applies to all functions inside.
#[mutants::exclude_re("replace .* -> bool")]
mod predicates {
    pub fn always_true() -> bool {
        true
    }

    pub fn increment(x: i32) -> i32 {
        x + 1
    }
}

/// exclude_re on an impl block is inherited by methods; methods can add their own.
pub struct Combo;

#[mutants::exclude_re("replace .* -> bool")]
impl Combo {
    /// This method adds its own exclude_re on top of the impl-level one.
    #[mutants::exclude_re("replace .* -> i32")]
    pub fn count(&self) -> i32 {
        1 + 2
    }

    pub fn is_ok(&self) -> bool {
        true
    }

    /// This method is not excluded by the impl-level pattern (returns i32, not bool).
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_numbers() {
        assert_eq!(add_numbers(2, 3), 5);
    }

    #[test]
    fn test_multiply() {
        assert_eq!(multiply(2, 3), 6);
    }

    #[test]
    fn test_subtract() {
        assert_eq!(subtract(5, 3), 2);
    }

    #[test]
    fn test_add_one() {
        assert_eq!(add_one(5), 6);
    }

    #[test]
    fn test_is_positive() {
        assert!(Calculator::is_positive(1));
        assert!(!Calculator::is_positive(-1));
    }

    #[test]
    fn test_double() {
        assert_eq!(Calculator::double(3), 6);
    }

    struct MyChecker;
    impl Checker for MyChecker {}

    #[test]
    fn test_checker_defaults() {
        let c = MyChecker;
        assert!(c.is_valid());
        assert_eq!(c.score(), 3);
    }

    #[test]
    fn test_predicates() {
        assert!(predicates::always_true());
        assert_eq!(predicates::increment(5), 6);
    }

    #[test]
    fn test_combo() {
        let c = Combo;
        assert_eq!(c.count(), 3);
        assert!(c.is_ok());
        assert_eq!(c.add(2, 3), 5);
    }
}

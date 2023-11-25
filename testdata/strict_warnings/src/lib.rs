//! Example of code with strict warnings that might fail to compile.

#![forbid(unused)]

pub fn some_fn(a: usize) -> usize {
    a + 2
}

#[cfg(test)]
mod test {
    #[test]
    fn test_some_fn() {
        assert_eq!(super::some_fn(10), 12);
    }
}

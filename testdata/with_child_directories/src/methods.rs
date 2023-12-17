pub fn double(x: usize) -> usize {
    x * 2
}

#[cfg(test)]
mod test {
    #[test]
    fn test_double() {
        assert_eq!(super::double(2), 4);
        assert_eq!(super::double(8), 16);
    }
}

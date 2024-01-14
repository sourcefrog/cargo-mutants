pub fn double(x: usize) -> usize {
    x * 2
}

#[cfg(test)]
mod test {
    #[test]
    fn double() {
        assert_eq!(super::double(2), 4);
        assert_eq!(super::double(0), 0);
        assert_eq!(super::double(6), 12);
    }
}

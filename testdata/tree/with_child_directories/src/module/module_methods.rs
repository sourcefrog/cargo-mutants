pub fn double(x: usize) -> usize {
    x * 2
}

#[cfg(test)]
mod test {
    #[test]
    fn double() {
        assert_eq!(super::double(2), 4);
    }
}

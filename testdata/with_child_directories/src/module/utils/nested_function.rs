fn has_nested() -> u32 {
    fn inner() -> u32 {
        12
    }
    inner() * inner()
}

#[cfg(test)]
mod test {
    #[test]
    fn has_nested() {
        assert_eq!(super::has_nested(), 144);
    }
}

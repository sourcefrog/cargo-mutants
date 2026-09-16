//! A tree whose tests allocate without bound when a particular mutant is applied.
//!
//! `replace big_enough -> bool with false` turns the loop in `grow_buffer` into an
//! unbounded allocator, which grows fast enough to hit a memory limit long before any
//! reasonable test timeout.

/// Is the buffer big enough to stop growing it?
fn big_enough(chunks: usize) -> bool {
    chunks >= 8
}

/// Grow a buffer a mebibyte at a time until it's big enough, and return its size in
/// mebibytes.
pub fn grow_buffer() -> usize {
    let mut buffer: Vec<Vec<u8>> = Vec::new();
    while !big_enough(buffer.len()) {
        // Written, not just reserved, so that the memory is really resident.
        buffer.push(vec![0xab; 1 << 20]);
    }
    buffer.len()
}

#[cfg(test)]
mod test {
    #[test]
    fn grow_buffer_stops_when_big_enough() {
        assert_eq!(super::grow_buffer(), 8);
    }
}

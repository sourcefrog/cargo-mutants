//! Show how to handle a struct with lifetime.

pub(crate) struct Lex<'buf> {
    buf: &'buf [u8],
    /// Position of the cursor within `buf`.
    pos: usize,
}

impl<'buf> Lex<'buf> {
    pub fn new(buf: &'buf [u8]) -> Lex<'buf> {
        Lex { buf, pos: 0 }
    }

    pub fn buf_len(&self) -> usize {
        self.buf.len()
    }
}

#[test]
fn get_as_slice() {
    let buf = b"hello";
    let lex = Lex::new(buf);
    assert_eq!(lex.buf_len(), 5);
}

pub fn binops() {
    let _ = 1 + 2 * 3 / 4 % 5;
    let _ = 1 & 2 | 3 ^ 4 << 5 >> 6;
    let mut a = 0isize;
    a += 1;
    a -= 2;
    a *= 3;
    a /= 2;
}

pub fn bin_assign() -> i32 {
    let mut a = 0;
    a |= 0xfff7;
    a ^= 0xffff;
    a &= 0x0f;
    a >>= 4;
    a <<= 1;
    a
}

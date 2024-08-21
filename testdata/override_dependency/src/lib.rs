#[mutants::skip]
pub fn hang() -> ! {
    loop {}
}

pub fn is_even(n: i32) -> bool {
    n % 2 == 0
}

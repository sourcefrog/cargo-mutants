use std::convert::TryInto;

use cargo_mutants_testdata_dependency::factorial;

pub fn double_factorial(n: i32) -> u32 {
    if n < 0 {
        return 0;
    }
    2 * factorial(n.try_into().unwrap())
}

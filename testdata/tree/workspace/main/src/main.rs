use cargo_mutants_testdata_workspace_utils::triple;

#[mutants::skip]
fn main() {
    for i in 1..=6 {
        println!("{}! = {}", i, factorial(i));
    }
    println!("3 * 3 = {}", triple(3));
}

fn factorial(n: u32) -> u32 {
    let mut a = 1;
    for i in 2..=n {
        a *= i;
    }
    a
}

#[test]
fn factorial_5() {
    assert_eq!(factorial(5), 120);
}

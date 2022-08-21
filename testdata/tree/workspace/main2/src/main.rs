use cargo_mutants_testdata_workspace_utils::triple;

#[mutants::skip]
fn main() {
    println!("Print works from main2 binary");
    println!("3 * 3 = {}", triple_3());
}

fn triple_3() -> i32 {
    triple(3)
}

mod test {
    use super::*;

    #[test]
    fn triple_3_test() {
        assert_eq!(triple_3(), 9);
    }
}

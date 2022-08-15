use std::fmt;

use cargo_mutants_testdata_workspace_utils::triple;

fn main() {
    println!("Print works from main2 binary");
    println!("3 * 3 = {}", triple(3));
}

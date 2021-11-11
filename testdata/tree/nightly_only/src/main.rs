#![feature(box_syntax)]
fn main() {
    let my_box = box 5;
    println!("{}", *my_box);
}

#[path = "foo.rs"]
pub mod c;

pub mod inline {
    #[path = "other.rs"]
    pub mod inner;
}

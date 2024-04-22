fn main() {}

/// Source file intentionally does not exist
#[cfg(not(test))] // allow tests to run successfully
mod nonexistent;

#[path = "../../nested_mod/src/paths_in_main/a/foo.rs"]
#[cfg(not(test))] // allow tests to run successfully
pub mod outside_workspace;

mod verify_continue {
    pub fn always_true() -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert!(super::verify_continue::always_true());
    }
}

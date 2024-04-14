pub mod block_in_lib {
    pub mod a {
        pub mod b {
            pub mod c_file;
        }
    }
}

pub mod file_in_lib;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert!(crate::file_in_lib::a::b::c_file::d::e::f_file::always_true());
        assert!(crate::block_in_lib::a::b::c_file::d::e::f_file::always_true());
    }
}

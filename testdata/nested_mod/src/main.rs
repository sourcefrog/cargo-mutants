fn main() {}

pub mod block_in_main {
    pub mod a {
        pub mod b {
            pub mod c_file;
        }
    }
}

pub mod file_in_main;

pub mod paths_in_main {
    //! Loosely follows naming from examples in the reference
    //! <https://doc.rust-lang.org/reference/items/modules.html#the-path-attribute>

    pub mod a {
        pub mod b;
    }
    pub mod a_mod_file;

    #[path = "thread_files"]
    pub mod thread {
        #[path = "tls.rs"]
        pub mod local_data;
    }

    pub mod thread_inner_attr {
        //! `path` can also be an inner attribute on `mod foo { ... }` blocks
        #![path = "thread_files_inner_attr"]

        #[path = "tls.rs"]
        pub mod local_data;
    }
}

#[path = "toplevel_file_in_main.rs"]
pub mod toplevel_in_main;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert!(crate::file_in_main::a::b::c_file::d::e::f_file::always_true());
        assert!(crate::block_in_main::a::b::c_file::d::e::f_file::always_true());

        assert!(crate::paths_in_main::a::b::c::always_true());
        assert!(crate::paths_in_main::a_mod_file::c::always_true());

        assert!(crate::paths_in_main::a::b::inline::inner::always_true());
        assert!(crate::paths_in_main::a_mod_file::inline::inner::always_true());

        assert!(crate::paths_in_main::thread::local_data::always_true());
        assert!(crate::paths_in_main::thread_inner_attr::local_data::always_true());

        assert!(crate::toplevel_in_main::always_true());
    }
}

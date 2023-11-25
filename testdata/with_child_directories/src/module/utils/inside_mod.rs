mod outer {
    mod inner {
        pub fn name() -> &'static str {
            "Bob"
        }

        #[test]
        fn test_name() {
            assert_eq!(name(), "Bob");
        }
    }
}

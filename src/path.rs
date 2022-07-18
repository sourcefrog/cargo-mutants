use camino::Utf8Path;

pub trait Utf8PathSlashes {
    fn to_slash_path(&self) -> String;
}

impl Utf8PathSlashes for Utf8Path {
    fn to_slash_path(&self) -> String {
        self.components()
            .map(|c| c.as_str())
            .filter(|c| !c.is_empty())
            .map(|c| if c == "/" || c == "\\" { "" } else { c })
            .collect::<Vec<_>>()
            .join("/")
    }
}

#[cfg(test)]
mod test {
    use camino::Utf8PathBuf;

    use super::Utf8PathSlashes;

    #[test]
    fn path_slashes_drops_empty_parts() {
        let mut path = Utf8PathBuf::from("/a/b/c/");
        path.push("d/e/f");
        assert_eq!(path.to_slash_path(), "/a/b/c/d/e/f");
    }
}

use camino::Utf8Path;

pub trait Utf8PathSlashes {
    fn to_slash_path(&self) -> String;
}

impl Utf8PathSlashes for Utf8Path {
    fn to_slash_path(&self) -> String {
        self.components()
            .map(|c| c.as_str())
            .collect::<Vec<_>>()
            .join("/")
    }
}

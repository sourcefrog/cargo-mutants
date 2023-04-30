// Copyright 2021-2023 Martin Pool

//! Mutations to source files, and inference of interesting mutations to apply.

use std::borrow::Cow;
use std::fmt;
use std::fs;

use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use serde::ser::{SerializeStruct, Serializer};
use serde::Serialize;
use similar::TextDiff;

use crate::build_dir::BuildDir;
use crate::source::SourceFile;
use crate::textedit::{replace_region, Span};

/// A comment marker inserted next to changes, so they can be easily found.
const MUTATION_MARKER_COMMENT: &str = "/* ~ changed by cargo-mutants ~ */";

/// A mutation applied to source code.
#[derive(Clone, Eq, PartialEq)]
pub struct Mutant {
    /// Which file is being mutated.
    pub source_file: Arc<SourceFile>,

    /// The function that's being mutated.
    pub function_name: Arc<String>,

    /// The return type of the function, as a fragment of Rust syntax.
    pub return_type: Arc<String>,

    /// The mutated textual region.
    pub span: Span,

    /// The replacement text.
    pub replacement: Cow<'static, str>,
}

impl Mutant {
    /// Return text of the whole file with the mutation applied.
    pub fn mutated_code(&self) -> String {
        replace_region(
            &self.source_file.code,
            &self.span.start,
            &self.span.end,
            &format!("{{\n{} {}\n}}\n", self.replacement, MUTATION_MARKER_COMMENT),
        )
    }

    /// Return the original code for the entire file affected by this mutation.
    pub fn original_code(&self) -> &str {
        &self.source_file.code
    }

    pub fn return_type(&self) -> &str {
        &self.return_type
    }

    /// Return a "file:line" description of the location of this mutation.
    pub fn describe_location(&self) -> String {
        format!(
            "{}:{}",
            self.source_file.tree_relative_slashes(),
            self.span.start.line,
        )
    }

    /// Describe the mutant briefly, not including the location.
    ///
    /// The result is like `replace factorial -> u32 with Default::default()`.
    pub fn describe_change(&self) -> String {
        format!(
            "replace {name}{space}{type} with {replacement}",
            name = self.function_name(),
            space = if self.return_type.is_empty() {
                ""
            } else {
                " "
            },
            type = self.return_type(),
            replacement = self.replacement
        )
    }

    /// Return the text inserted for this mutation.
    pub fn replacement_text(&self) -> &str {
        self.replacement.as_ref()
    }

    /// Return the name of the function to be mutated.
    ///
    /// Note that this will often not be unique: the same name can be reused
    /// in different modules, under different cfg guards, etc.
    pub fn function_name(&self) -> &str {
        &self.function_name
    }

    /// Return the cargo package name.
    pub fn package_name(&self) -> &str {
        &self.source_file.package_name
    }

    /// Return a unified diff for the mutant.
    pub fn diff(&self) -> String {
        let old_label = self.source_file.tree_relative_slashes();
        // There shouldn't be any newlines, but just in case...
        let new_label = self.describe_change().replace('\n', " ");
        TextDiff::from_lines(self.original_code(), &self.mutated_code())
            .unified_diff()
            .context_radius(8)
            .header(&old_label, &new_label)
            .to_string()
    }

    pub fn apply(&self, build_dir: &BuildDir) -> Result<()> {
        self.write_in_dir(build_dir, &self.mutated_code())
    }

    pub fn unapply(&self, build_dir: &BuildDir) -> Result<()> {
        self.write_in_dir(build_dir, self.original_code())
    }

    fn write_in_dir(&self, build_dir: &BuildDir, code: &str) -> Result<()> {
        let path = self
            .source_file
            .tree_relative_path()
            .within(build_dir.path());
        // for safety, don't follow symlinks
        assert!(path.is_file(), "{path:?} is not a file");
        fs::write(&path, code.as_bytes())
            .with_context(|| format!("failed to write mutated code to {path:?}"))
    }

    pub fn log_file_name_base(&self) -> String {
        format!(
            "{}_line_{}",
            self.source_file.tree_relative_slashes(),
            self.span.start.line
        )
    }
}

impl fmt::Debug for Mutant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Mutant")
            .field("function_name", &self.function_name())
            .field("return_type", &self.return_type)
            .field("replacement", &self.replacement)
            // more concise display of spans
            .field("start", &(self.span.start.line, self.span.start.column))
            .field("end", &(self.span.end.line, self.span.end.column))
            .field("package_name", &self.package_name())
            .finish()
    }
}

impl fmt::Display for Mutant {
    /// Describe this mutant like a compiler error message, starting with the file and line.
    ///
    /// The result is like `src/source.rs:123: replace source::SourceFile::new with Default::default()`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // This is like `style_mutant`, but without colors.
        // The text content should be the same.
        write!(
            f,
            "{file}:{line}: {change}",
            file = self.source_file.tree_relative_slashes(),
            line = self.span.start.line,
            change = self.describe_change()
        )
    }
}

impl Serialize for Mutant {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // custom serialize to omit inessential info
        let mut ss = serializer.serialize_struct("Mutation", 6)?;
        ss.serialize_field("package", &self.package_name())?;
        ss.serialize_field("file", &self.source_file.tree_relative_slashes())?;
        ss.serialize_field("line", &self.span.start.line)?;
        ss.serialize_field("function", &self.function_name.as_ref())?;
        ss.serialize_field("return_type", &self.return_type.as_ref())?;
        ss.serialize_field("replacement", self.replacement.as_ref())?;
        ss.end()
    }
}

#[cfg(test)]
mod test {
    use camino::Utf8Path;
    use indoc::indoc;
    use itertools::Itertools;
    use pretty_assertions::assert_eq;

    use crate::*;

    #[test]
    fn discover_factorial_mutants() {
        let tree_path = Utf8Path::new("testdata/tree/factorial");
        let tool = CargoTool::new();
        let source_tree = tool.find_root(tree_path).unwrap();
        let options = Options::default();
        let mutants = walk_tree(&tool, &source_tree, &options).unwrap().mutants;
        assert_eq!(mutants.len(), 2);
        assert_eq!(
            format!("{:?}", mutants[0]),
            r#"Mutant { function_name: "main", return_type: "", replacement: "()", start: (1, 11), end: (5, 2), package_name: "cargo-mutants-testdata-factorial" }"#
        );
        assert_eq!(
            mutants[0].to_string(),
            "src/bin/factorial.rs:1: replace main with ()"
        );
        assert_eq!(
            format!("{:?}", mutants[1]),
            r#"Mutant { function_name: "factorial", return_type: "-> u32", replacement: "Default::default()", start: (7, 29), end: (13, 2), package_name: "cargo-mutants-testdata-factorial" }"#
        );
        assert_eq!(
            mutants[1].to_string(),
            "src/bin/factorial.rs:7: replace factorial -> u32 with Default::default()"
        );
    }

    #[test]
    fn filter_by_attributes() {
        let tree_path = Utf8Path::new("testdata/tree/hang_avoided_by_attr");
        let tool = CargoTool::new();
        let source_tree = tool.find_root(tree_path).unwrap();
        let mutants = walk_tree(&tool, &source_tree, &Options::default())
            .unwrap()
            .mutants;
        let descriptions = mutants.iter().map(Mutant::describe_change).collect_vec();
        insta::assert_snapshot!(
            descriptions.join("\n"),
            @"replace controlled_loop with ()"
        );
    }

    #[test]
    fn mutate_factorial() {
        let tree_path = Utf8Path::new("testdata/tree/factorial");
        let tool = CargoTool::new();
        let source_tree = tool.find_root(tree_path).unwrap();
        let mutants = walk_tree(&tool, &source_tree, &Options::default())
            .unwrap()
            .mutants;
        assert_eq!(mutants.len(), 2);

        let mut mutated_code = mutants[0].mutated_code();
        assert_eq!(mutants[0].function_name(), "main");
        mutated_code.retain(|c| c != '\r');
        assert_eq!(
            mutated_code,
            indoc! { r#"
                fn main() {
                () /* ~ changed by cargo-mutants ~ */
                }

                fn factorial(n: u32) -> u32 {
                    let mut a = 1;
                    for i in 2..=n {
                        a *= i;
                    }
                    a
                }

                #[test]
                fn test_factorial() {
                    println!("factorial({}) = {}", 6, factorial(6)); // This line is here so we can see it in --nocapture
                    assert_eq!(factorial(6), 720);
                }
                "#
            }
        );

        let mut mutated_code = mutants[1].mutated_code();
        assert_eq!(mutants[1].function_name(), "factorial");
        mutated_code.retain(|c| c != '\r');
        assert_eq!(
            mutated_code,
            indoc! { r#"
                fn main() {
                    for i in 1..=6 {
                        println!("{}! = {}", i, factorial(i));
                    }
                }

                fn factorial(n: u32) -> u32 {
                Default::default() /* ~ changed by cargo-mutants ~ */
                }

                #[test]
                fn test_factorial() {
                    println!("factorial({}) = {}", 6, factorial(6)); // This line is here so we can see it in --nocapture
                    assert_eq!(factorial(6), 720);
                }
                "#
            }
        );
    }
}

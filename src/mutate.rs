// Copyright 2021-2023 Martin Pool

//! Mutations to source files, and inference of interesting mutations to apply.

use std::fmt;
use std::fmt::Write;
use std::fs;
use std::sync::Arc;

use anyhow::{ensure, Context, Result};
use console::style;
use serde::ser::{SerializeStruct, Serializer};
use serde::Serialize;
use similar::TextDiff;

use crate::build_dir::BuildDir;
use crate::package::Package;
use crate::source::SourceFile;
use crate::textedit::span_substr;
use crate::textedit::{replace_region, Span};
use crate::MUTATION_MARKER_COMMENT;

/// Various broad categories of mutants.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub enum Genre {
    /// Replace the body of a function with a fixed value.
    FnValue,
    /// Replace `==` with `!=` and so on.
    BinaryOperator,
}

/// A mutation applied to source code.
#[derive(Clone, Eq, PartialEq)]
pub struct Mutant {
    /// Which file is being mutated.
    pub source_file: Arc<SourceFile>,

    /// The function that's being mutated: the nearest enclosing function, if they are nested.
    pub function: Arc<Function>,

    /// The primary start line for this mutant, shown in single line output
    /// like `src/foo.rs:123: replace foo with bar`.
    ///
    /// This is the line that makes most sense for the user to visit to see
    /// the mutated code. It might not overlap with the mutation span: specifically
    /// for FnValue mutants this points to the line with the function ident,
    /// not the body span.
    pub primary_line: usize,

    /// The mutated textual region.
    ///
    /// This is deleted and replaced with the replacement text.
    pub span: Span,

    /// The replacement text.
    pub replacement: String,

    /// What general category of mutant this is.
    pub genre: Genre,
}

/// The function containing a mutant.
///
/// This is used for both mutations of the whole function, and smaller mutations within it.
#[derive(Eq, PartialEq, Debug, Serialize)]
pub struct Function {
    /// The function that's being mutated.
    pub function_name: String,

    /// The return type of the function, including a leading "-> ", as a fragment of Rust syntax.
    ///
    /// Empty if the function has no return type (i.e. returns `()`).
    pub return_type: String,

    /// The span (line/column range) of the entire function.
    pub span: Span,
}

impl Mutant {
    /// Return text of the whole file with the mutation applied.
    pub fn mutated_code(&self) -> String {
        replace_region(
            &self.source_file.code,
            &self.span.start,
            &self.span.end,
            &format!("{} {} ", &self.replacement, MUTATION_MARKER_COMMENT),
        )
    }

    /// Return the original code for the entire file affected by this mutation.
    pub fn original_code(&self) -> &str {
        &self.source_file.code
    }

    pub fn return_type(&self) -> &str {
        &self.function.return_type
    }

    /// Describe the mutant briefly, not including the location.
    ///
    /// The result is like `replace factorial -> u32 with Default::default()`.
    pub fn describe_change(&self) -> String {
        // TODO: This needs to be updated for smaller-than-function mutations.
        if self.genre == Genre::FnValue {
            format!(
                "replace {name}{space}{type} with {replacement}",
                name = self.function.function_name,
                space = if self.function.return_type.is_empty() {
                    ""
                } else {
                    " "
                },
                type = self.function.return_type,
                replacement = self.replacement,
            )
        } else {
            format!(
                "replace {original} with {replacement} in {name}{space}{type}",
                original = self.original_text(),
                replacement = self.replacement,
                name = self.function.function_name,
                space = if self.function.return_type.is_empty() {
                    ""
                } else {
                    " "
                },
                type = self.function.return_type,
            )
        }
    }

    pub fn styled(&self) -> String {
        // This is like `impl Display for Mutant`, but with colors.
        // The text content should be the same.
        // TODO: implement Display by stripping out the styles.
        let mut s = String::with_capacity(200);
        write!(
            &mut s,
            "{}:{}",
            self.source_file.tree_relative_slashes(),
            self.primary_line,
        )
        .unwrap();
        s.push_str(": replace ");
        if self.genre != Genre::FnValue {
            // TODO: colors
            s.push_str(&self.original_text());
            s.push_str(" with ");
            s.push_str(&self.replacement);
            s.push_str(" in ");
        }
        s.push_str(&style(self.function_name()).bright().magenta().to_string());
        if self.genre == Genre::FnValue {
            if !self.return_type().is_empty() {
                s.push(' ');
                s.push_str(&style(self.return_type()).magenta().to_string());
            }
            s.push_str(" with ");
            s.push_str(&style(self.replacement_text()).yellow().to_string());
        }
        s
    }

    pub fn original_text(&self) -> String {
        span_substr(&self.source_file.code, &self.span)
    }

    /// Return the text inserted for this mutation.
    pub fn replacement_text(&self) -> &str {
        self.replacement.as_str()
    }

    /// Return the name of the function to be mutated.
    ///
    /// Note that this will often not be unique: the same name can be reused
    /// in different modules, under different cfg guards, etc.
    pub fn function_name(&self) -> &str {
        &self.function.function_name
    }

    /// Return the cargo package name.
    pub fn package_name(&self) -> &str {
        &self.source_file.package.name
    }

    pub fn package(&self) -> &Package {
        &self.source_file.package
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

    /// Apply this mutant to the relevant file within a BuildDir.
    pub fn apply(&self, build_dir: &mut BuildDir) -> Result<()> {
        self.write_in_dir(build_dir, &self.mutated_code())
    }

    pub fn unapply(&self, build_dir: &mut BuildDir) -> Result<()> {
        self.write_in_dir(build_dir, self.original_code())
    }

    #[allow(unknown_lints, clippy::needless_pass_by_ref_mut)]
    // The Rust object is not mutated, but the BuildDir on disk should be exclusively owned for this to be safe.
    fn write_in_dir(&self, build_dir: &mut BuildDir, code: &str) -> Result<()> {
        let path = build_dir.path().join(&self.source_file.tree_relative_path);
        // for safety, don't follow symlinks
        ensure!(path.is_file(), "{path:?} is not a file");
        fs::write(&path, code.as_bytes())
            .with_context(|| format!("failed to write mutated code to {path:?}"))
    }

    pub fn log_file_name_base(&self) -> String {
        format!(
            "{}_line_{}",
            self.source_file.tree_relative_slashes(),
            self.primary_line,
        )
    }
}

impl fmt::Debug for Mutant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Custom implementation to show spans more concisely
        f.debug_struct("Mutant")
            .field("start_line", &self.primary_line)
            .field("function", &self.function)
            .field("replacement", &self.replacement)
            .field("genre", &self.genre)
            .field("span", &self.span)
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
            line = self.primary_line,
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
        let mut ss = serializer.serialize_struct("Mutant", 7)?;
        let function: &Function = self.function.as_ref();
        ss.serialize_field("package", &self.package_name())?;
        ss.serialize_field("file", &self.source_file.tree_relative_slashes())?;
        ss.serialize_field("line", &self.primary_line)?;
        ss.serialize_field("function", function)?;
        ss.serialize_field("span", &self.span)?;
        ss.serialize_field("replacement", &self.replacement)?;
        ss.serialize_field("genre", &self.genre)?;
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
        let tree_path = Utf8Path::new("testdata/factorial");
        let workspace = Workspace::open(tree_path).unwrap();
        let options = Options::default();
        let mutants = workspace
            .mutants(&PackageFilter::All, &options, &Console::new())
            .unwrap();
        assert_eq!(mutants.len(), 3);
        assert_eq!(
            format!("{:#?}", mutants[0]),
            indoc! {
                r#"Mutant {
                    start_line: 1,
                    function: Function {
                        function_name: "main",
                        return_type: "",
                        span: Span {
                            start: LineColumn(1, 1),
                            end: LineColumn(5, 2),
                        },
                    },
                    replacement: "()",
                    genre: FnValue,
                    span: Span {
                        start: LineColumn(2, 5),
                        end: LineColumn(4, 6),
                    },
                    package_name: "cargo-mutants-testdata-factorial",
                }"#
            }
        );
        assert_eq!(
            mutants[0].to_string(),
            "src/bin/factorial.rs:1: replace main with ()"
        );
        assert_eq!(
            format!("{:#?}", mutants[1]),
            indoc! { r#"
                Mutant {
                    start_line: 7,
                    function: Function {
                        function_name: "factorial",
                        return_type: "-> u32",
                        span: Span {
                            start: LineColumn(7, 1),
                            end: LineColumn(13, 2),
                        },
                    },
                    replacement: "0",
                    genre: FnValue,
                    span: Span {
                        start: LineColumn(8, 5),
                        end: LineColumn(12, 6),
                    },
                    package_name: "cargo-mutants-testdata-factorial",
                }"#
            }
        );
        assert_eq!(
            mutants[1].to_string(),
            "src/bin/factorial.rs:7: replace factorial -> u32 with 0"
        );
        assert_eq!(
            mutants[2].to_string(),
            "src/bin/factorial.rs:7: replace factorial -> u32 with 1"
        );
    }

    #[test]
    fn filter_by_attributes() {
        let mutants = Workspace::open(Utf8Path::new("testdata/hang_avoided_by_attr"))
            .unwrap()
            .mutants(&PackageFilter::All, &Options::default(), &Console::new())
            .unwrap();
        let descriptions = mutants.iter().map(Mutant::describe_change).collect_vec();
        insta::assert_snapshot!(
            descriptions.join("\n"),
            @"replace controlled_loop with ()"
        );
    }

    #[test]
    fn mutate_factorial() -> Result<()> {
        let tree_path = Utf8Path::new("testdata/factorial");
        let mutants = Workspace::open(tree_path)?.mutants(
            &PackageFilter::All,
            &Options::default(),
            &Console::new(),
        )?;
        assert_eq!(mutants.len(), 3);

        let mut mutated_code = mutants[0].mutated_code();
        assert_eq!(mutants[0].function_name(), "main");
        mutated_code.retain(|c| c != '\r');
        assert_eq!(
            mutated_code,
            indoc! { r#"
                fn main() {
                    () /* ~ changed by cargo-mutants ~ */ }

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
                    0 /* ~ changed by cargo-mutants ~ */ }

                #[test]
                fn test_factorial() {
                    println!("factorial({}) = {}", 6, factorial(6)); // This line is here so we can see it in --nocapture
                    assert_eq!(factorial(6), 720);
                }
                "#
            }
        );
        Ok(())
    }
}

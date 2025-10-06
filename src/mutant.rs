// Copyright 2021 - 2025 Martin Pool

//! Mutations to source files, and inference of interesting mutations to apply.

use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

use anyhow::Result;
use console::{style, StyledObject};
use serde::ser::{SerializeStruct, Serializer};
use serde::Serialize;
use similar::TextDiff;
use tracing::trace;

use crate::build_dir::BuildDir;
use crate::output::clean_filename;
use crate::source::SourceFile;
use crate::span::Span;
use crate::MUTATION_MARKER_COMMENT;

/// Various broad categories of mutants.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub enum Genre {
    /// Replace the body of a function with a fixed value.
    FnValue,
    /// Replace `==` with `!=` and so on.
    BinaryOperator,
    UnaryOperator,
    /// Delete match arm.
    MatchArm,
    /// Replace the expression of a match arm guard with a fixed value.
    MatchArmGuard,
    /// Delete a field from a struct literal that has a base (default) expression.
    StructField,
}

/// A mutation applied to source code.
#[derive(Clone, Eq, PartialEq)]
pub struct Mutant {
    /// Which file is being mutated.
    pub source_file: SourceFile,

    /// The function that's being mutated: the nearest enclosing function, if they are nested.
    ///
    /// There may be none for mutants in e.g. top-level const expressions.
    pub function: Option<Arc<Function>>,

    /// The location of the mutated textual region in the original source file.
    ///
    /// This is deleted and replaced with the replacement text.
    ///
    /// This may be long, for example when a whole function body is replaced. This is used primarily to
    /// show the line/col location of the mutation.
    pub span: Span,

    /// A shorter version of the text being replaced.
    ///
    /// For example, when a match arm is replaced, this gives only the match pattern, not the
    /// body of the arm.
    pub short_replaced: Option<String>,

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
    /// The function that's being mutated, including any containing namespaces.
    #[allow(clippy::struct_field_names)]
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
        self.span.replace(
            self.source_file.code(),
            &format!("{} {}", &self.replacement, MUTATION_MARKER_COMMENT),
        )
    }

    /// Describe the mutant briefly, not including the location.
    ///
    /// The result is like `replace factorial -> u32 with Default::default()`.
    pub fn describe_change(&self) -> String {
        self.styled_parts()
            .into_iter()
            .map(|x| x.force_styling(false).to_string())
            .collect::<String>()
    }

    pub fn name(&self, show_line_col: bool) -> String {
        let mut v = Vec::new();
        v.push(self.source_file.tree_relative_slashes());
        if show_line_col {
            v.push(format!(
                ":{}:{}: ",
                self.span.start.line, self.span.start.column
            ));
        } else {
            v.push(": ".to_owned());
        }
        v.extend(
            self.styled_parts()
                .into_iter()
                .map(|x| x.force_styling(false).to_string()),
        );
        v.join("")
    }

    /// Return a one-line description of this mutant, with coloring, including the file names
    /// and optionally the line and column.
    pub fn to_styled_string(&self, show_line_col: bool) -> String {
        let mut v = Vec::new();
        v.push(self.source_file.tree_relative_slashes());
        if show_line_col {
            v.push(format!(
                ":{}:{}",
                self.span.start.line, self.span.start.column
            ));
        }
        v.push(": ".to_owned());
        v.extend(self.styled_parts().into_iter().map(|x| x.to_string()));
        v.join("")
    }

    fn styled_parts(&self) -> Vec<StyledObject<String>> {
        // This is like `impl Display for Mutant`, but with colors.
        // The text content should be the same.
        #[allow(clippy::needless_pass_by_value)] // actually is needed for String vs &str?
        fn s<S: ToString>(s: S) -> StyledObject<String> {
            style(s.to_string())
        }
        let mut v: Vec<StyledObject<String>> = Vec::new();
        match self.genre {
            Genre::FnValue => {
                v.push(s("replace "));
                let function = self
                    .function
                    .as_ref()
                    .expect("FnValue mutant should have a function");
                v.push(s(&function.function_name).bright().magenta());
                if !function.return_type.is_empty() {
                    v.push(s(" "));
                    v.push(s(&function.return_type).magenta());
                }
                v.push(s(" with "));
                v.push(s(self.replacement_text()).yellow());
            }
            Genre::MatchArmGuard => {
                v.push(s("replace match guard "));
                v.push(s(squash_lines(self.original_text().as_ref())).yellow());
                v.push(s(" with "));
                v.push(s(self.replacement_text()).yellow());
            }
            Genre::MatchArm => {
                v.push(s("delete match arm "));
                v.push(
                    s(squash_lines(
                        self.short_replaced
                            .as_ref()
                            .expect("short_replaced should be set on MatchArm"),
                    ))
                    .yellow(),
                );
            }
            Genre::StructField => {
                let field_and_type = self
                    .short_replaced
                    .as_ref()
                    .expect("short_replaced should be set on StructField");
                // Parse "field_name::StructType" format
                if let Some((field_name, struct_type)) = field_and_type.split_once("::") {
                    v.push(s("delete field "));
                    v.push(s(field_name).yellow());
                    v.push(s(" from struct "));
                    v.push(s(struct_type).yellow());
                    v.push(s(" expression"));
                } else {
                    // Fallback for older format (shouldn't happen)
                    v.push(s("delete field "));
                    v.push(s(field_and_type).yellow());
                    v.push(s(" from struct expression"));
                }
            }
            _ => {
                if self.replacement.is_empty() {
                    v.push(s("delete "));
                } else {
                    v.push(s("replace "));
                }
                v.push(s(self.original_text()).yellow());
                if !self.replacement.is_empty() {
                    v.push(s(" with "));
                    v.push(s(&self.replacement).bright().yellow());
                }
            }
        }
        if !matches!(self.genre, Genre::FnValue) {
            if let Some(func) = &self.function {
                v.push(s(" in "));
                v.push(s(&func.function_name).bright().magenta());
            }
        }
        v
    }

    pub fn original_text(&self) -> String {
        self.span.extract(self.source_file.code())
    }

    /// Return the text inserted for this mutation.
    pub fn replacement_text(&self) -> &str {
        self.replacement.as_str()
    }

    /// Return a unified diff for the mutant.
    ///
    /// The mutated text must be passed in because we should have already computed
    /// it, and don't want to pointlessly recompute it here.
    pub fn diff(&self, mutated_code: &str) -> String {
        let old_label = self.source_file.tree_relative_slashes();
        // There shouldn't be any newlines, but just in case...
        let new_label = self.describe_change().replace('\n', " ");
        TextDiff::from_lines(self.source_file.code(), mutated_code)
            .unified_diff()
            .context_radius(8)
            .header(&old_label, &new_label)
            .to_string()
    }

    /// Apply this mutant to the relevant file within a `BuildDir`.
    pub fn apply(&self, build_dir: &BuildDir, mutated_code: &str) -> Result<()> {
        trace!(?self, "Apply mutant");
        build_dir.overwrite_file(&self.source_file.tree_relative_path, mutated_code)
    }

    pub fn revert(&self, build_dir: &BuildDir) -> Result<()> {
        trace!(?self, "Revert mutant");
        build_dir.overwrite_file(
            &self.source_file.tree_relative_path,
            self.source_file.code(),
        )
    }

    /// Return a string describing this mutant that's suitable for building a log file name,
    /// but can contain slashes.
    pub fn log_file_name_base(&self) -> String {
        // TODO: Also include a unique number so that they can't collide, even
        // with similar mutants on the same line?
        format!(
            "{filename}_line_{line}_col_{col}",
            filename = clean_filename(&self.source_file.tree_relative_slashes()),
            line = self.span.start.line,
            col = self.span.start.column,
        )
    }
}

impl fmt::Debug for Mutant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Custom implementation to show spans more concisely
        f.debug_struct("Mutant")
            .field("function", &self.function)
            .field("replacement", &self.replacement)
            .field("genre", &self.genre)
            .field("span", &self.span)
            .field("short_replaced", &self.short_replaced)
            .field("package_name", &self.source_file.package.name)
            .finish()
    }
}

impl Serialize for Mutant {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // custom serialize to omit inessential info
        let mut ss = serializer.serialize_struct("Mutant", 7)?;
        ss.serialize_field("package", &self.source_file.package.name)?;
        ss.serialize_field("file", &self.source_file.tree_relative_slashes())?;
        ss.serialize_field("function", &self.function.as_ref().map(Arc::as_ref))?;
        ss.serialize_field("span", &self.span)?;
        ss.serialize_field("replacement", &self.replacement)?;
        ss.serialize_field("genre", &self.genre)?;
        ss.end()
    }
}

/// Combine multiple lines to one, removing indentation following a newline.
///
/// Newlines are replaced by a space, only if there is not already a trailing space.
pub fn squash_lines(s: &str) -> Cow<'_, str> {
    if s.contains('\n') {
        let mut r = String::new();
        let mut in_indent = false;
        for c in s.chars() {
            match c {
                ' ' | '\t' | '\n' if in_indent => (),
                '\n' => {
                    if !r.ends_with(' ') {
                        r.push(' ');
                    }
                    in_indent = true;
                }
                c => {
                    in_indent = false;
                    r.push(c);
                }
            }
        }
        Cow::Owned(r)
    } else {
        Cow::Borrowed(s)
    }
}

#[cfg(test)]
mod test {
    use indoc::indoc;
    use itertools::Itertools;
    use pretty_assertions::assert_eq;

    use crate::test_util::copy_of_testdata;
    use crate::visit::mutate_source_str;
    use crate::*;

    #[test]
    fn squash_lines() {
        use super::squash_lines;
        assert_eq!(squash_lines("squash_lines a b c"), "squash_lines a b c");
        assert_eq!(squash_lines("a\n    b c \n\nd  \n  e"), "a b c d  e");
    }

    #[test]
    fn discover_factorial_mutants() {
        let tmp = copy_of_testdata("factorial");
        let workspace = Workspace::open(tmp.path()).unwrap();
        let options = Options::default();
        let mutants = workspace
            .discover(&PackageFilter::All, &options, &Console::new())
            .unwrap()
            .mutants;
        assert_eq!(mutants.len(), 5);
        assert_eq!(
            format!("{:#?}", mutants[0]),
            indoc! {
                r#"Mutant {
                    function: Some(
                        Function {
                            function_name: "main",
                            return_type: "",
                            span: Span(1, 1, 5, 2),
                        },
                    ),
                    replacement: "()",
                    genre: FnValue,
                    span: Span(2, 5, 4, 6),
                    short_replaced: None,
                    package_name: "cargo-mutants-testdata-factorial",
                }"#
            }
        );
        assert_eq!(
            mutants[0].name(true),
            "src/bin/factorial.rs:2:5: replace main with ()"
        );
        assert_eq!(
            format!("{:#?}", mutants[1]),
            indoc! { r#"
                Mutant {
                    function: Some(
                        Function {
                            function_name: "factorial",
                            return_type: "-> u32",
                            span: Span(7, 1, 13, 2),
                        },
                    ),
                    replacement: "0",
                    genre: FnValue,
                    span: Span(8, 5, 12, 6),
                    short_replaced: None,
                    package_name: "cargo-mutants-testdata-factorial",
                }"#
            }
        );
        assert_eq!(
            mutants[1].name(false),
            "src/bin/factorial.rs: replace factorial -> u32 with 0"
        );
        assert_eq!(
            mutants[1].name(true),
            "src/bin/factorial.rs:8:5: replace factorial -> u32 with 0"
        );
        assert_eq!(
            mutants[2].name(true),
            "src/bin/factorial.rs:8:5: replace factorial -> u32 with 1"
        );
    }

    #[test]
    fn filter_by_attributes() {
        let tmp = copy_of_testdata("hang_avoided_by_attr");
        let mutants = Workspace::open(tmp.path())
            .unwrap()
            .discover(&PackageFilter::All, &Options::default(), &Console::new())
            .unwrap()
            .mutants;
        let descriptions = mutants.iter().map(Mutant::describe_change).collect_vec();
        assert_eq!(
            descriptions,
            [
                "replace controlled_loop with ()",
                "replace > with == in controlled_loop",
                "replace > with < in controlled_loop",
                "replace > with >= in controlled_loop",
                "replace * with + in controlled_loop",
                "replace * with / in controlled_loop",
            ]
        );
    }

    #[test]
    fn always_skip_constructors_called_new() {
        let code = indoc! { r"
            struct S {
                x: i32,
            }

            impl S {
                fn new(x: i32) -> Self {
                    Self { x }
                }
            }
        " };
        let mutants = mutate_source_str(code, &Options::default()).unwrap();
        assert_eq!(mutants, []);
    }

    #[test]
    fn mutate_factorial() -> Result<()> {
        let temp = copy_of_testdata("factorial");
        let tree_path = temp.path();
        let mutants = Workspace::open(tree_path)?
            .discover(&PackageFilter::All, &Options::default(), &Console::new())?
            .mutants;
        assert_eq!(mutants.len(), 5);

        let mutated_code = mutants[0].mutated_code();
        assert_eq!(mutants[0].function.as_ref().unwrap().function_name, "main");
        assert_eq!(
            strip_trailing_space(&mutated_code),
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

        let mutated_code = mutants[1].mutated_code();
        assert_eq!(
            mutants[1].function.as_ref().unwrap().function_name,
            "factorial"
        );
        assert_eq!(
            strip_trailing_space(&mutated_code),
            indoc! { r#"
                fn main() {
                    for i in 1..=6 {
                        println!("{}! = {}", i, factorial(i));
                    }
                }

                fn factorial(n: u32) -> u32 {
                    0 /* ~ changed by cargo-mutants ~ */
                }

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

    fn strip_trailing_space(s: &str) -> String {
        // Split on \n so that we retain empty lines etc
        s.split('\n').map(str::trim_end).join("\n")
    }
}

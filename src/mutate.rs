// Copyright 2021, 2022 Martin Pool

//! Mutations to source files, and inference of interesting mutations to apply.

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

/// A type of mutation operation that could be applied to a source file.
#[derive(Debug, Eq, Clone, PartialEq, Serialize)]
pub enum MutationOp {
    /// Return [Default::default].
    Default,
    /// Replace the function body with nothing (for functions that return `()`.
    ///
    /// We use `()` rather than just nothing because it's clearer in messages about the mutation.
    Unit,
    /// Return true.
    True,
    /// Return false.
    False,
    /// Return empty string.
    EmptyString,
    /// Return `"xyzzy"`.
    Xyzzy,
    /// Return `Ok(Default::default())`
    OkDefault,
}

impl MutationOp {
    /// Return the text that replaces the body of the mutated span, without the marker comment.
    fn replacement(&self) -> &'static str {
        use MutationOp::*;
        match self {
            Default => "Default::default()",
            Unit => "()",
            True => "true",
            False => "false",
            EmptyString => "\"\".into()",
            Xyzzy => "\"xyzzy\".into()",
            OkDefault => "Ok(Default::default())",
        }
    }
}

/// A mutation applied to source code.
#[derive(Clone, Eq, PartialEq)]
pub struct Mutant {
    /// Which file is being mutated.
    source_file: Arc<SourceFile>,

    /// The function that's being mutated.
    function_name: Arc<String>,

    /// The return type of the function, as a fragment of Rust syntax.
    return_type: Arc<String>,

    /// The mutated textual region.
    span: Span,

    /// The type of change to apply.
    pub op: MutationOp,
}

impl Mutant {
    pub fn new(
        source_file: &Arc<SourceFile>,
        op: MutationOp,
        function_name: &Arc<String>,
        return_type: &Arc<String>,
        span: Span,
    ) -> Mutant {
        Mutant {
            source_file: Arc::clone(source_file),
            op,
            function_name: Arc::clone(function_name),
            return_type: Arc::clone(return_type),
            span,
        }
    }

    /// Return text of the whole file with the mutation applied.
    pub fn mutated_code(&self) -> String {
        replace_region(
            &self.source_file.code,
            &self.span.start,
            &self.span.end,
            &format!(
                "{{\n{} {}\n}}\n",
                self.op.replacement(),
                MUTATION_MARKER_COMMENT
            ),
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
    pub fn describe_change(&self) -> String {
        format!(
            "replace {} with {}",
            self.function_name(),
            self.op.replacement()
        )
    }

    /// Return the text inserted for this mutation.
    pub fn replacement_text(&self) -> &'static str {
        self.op.replacement()
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
        let new_label = self.describe_change();
        TextDiff::from_lines(self.original_code(), &self.mutated_code())
            .unified_diff()
            .context_radius(8)
            .header(&old_label, &new_label)
            .to_string()
    }

    /// Run a function with this mutation applied, then revert it afterwards, even if the function
    /// returns an error.
    pub fn with_mutation_applied<F, T>(&self, build_dir: &BuildDir, mut func: F) -> Result<T>
    where
        F: FnMut() -> Result<T>,
    {
        self.write_in_dir(build_dir, &self.mutated_code())?;
        let r = func();
        self.write_in_dir(build_dir, self.original_code())?;
        r
    }

    fn write_in_dir(&self, build_dir: &BuildDir, code: &str) -> Result<()> {
        let path = self
            .source_file
            .tree_relative_path()
            .within(build_dir.path());
        // for safety, don't follow symlinks
        assert!(path.is_file(), "{:?} is not a file", path);
        fs::write(&path, code.as_bytes())
            .with_context(|| format!("failed to write mutated code to {:?}", path))
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
            .field("op", &self.op)
            .field("function_name", &self.function_name())
            .field("return_type", &self.return_type)
            // more concise display of spans
            .field("start", &(self.span.start.line, self.span.start.column))
            .field("end", &(self.span.end.line, self.span.end.column))
            .field("package_name", &self.package_name())
            .finish()
    }
}

impl fmt::Display for Mutant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} in {}",
            self.describe_change(),
            self.describe_location(),
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
        ss.serialize_field("replacement", self.op.replacement())?;
        ss.end()
    }
}

#[cfg(test)]
mod test {
    use std::rc::Rc;

    use camino::Utf8Path;
    use itertools::Itertools;
    use pretty_assertions::assert_eq;

    use crate::*;

    #[test]
    fn discover_factorial_mutants() {
        let source_file = SourceFile::new(
            Utf8Path::new("testdata/tree/factorial"),
            "src/bin/factorial.rs".parse().unwrap(),
            Rc::new("cargo-mutants-testdata-factorial".to_string()),
        )
        .unwrap();
        let muts = discover_mutants(source_file.into()).unwrap();
        assert_eq!(muts.len(), 2);
        assert_eq!(
            format!("{:?}", muts[0]),
            r#"Mutant { op: Unit, function_name: "main", return_type: "", start: (1, 11), end: (5, 2), package_name: "cargo-mutants-testdata-factorial" }"#
        );
        assert_eq!(
            muts[0].to_string(),
            "replace main with () in src/bin/factorial.rs:1"
        );
        assert_eq!(
            format!("{:?}", muts[1]),
            r#"Mutant { op: Default, function_name: "factorial", return_type: "-> u32", start: (7, 29), end: (13, 2), package_name: "cargo-mutants-testdata-factorial" }"#
        );
        assert_eq!(
            muts[1].to_string(),
            "replace factorial with Default::default() in src/bin/factorial.rs:7"
        );
    }

    #[test]
    fn filter_by_attributes() {
        let source_file = SourceFile::new(
            Utf8Path::new("testdata/tree/hang_avoided_by_attr"),
            "src/lib.rs".parse().unwrap(),
            Rc::new("cargo-mutants-testdata-hang-avoided".to_string()),
        )
        .unwrap();
        let mutants = discover_mutants(source_file.into()).unwrap();
        let descriptions = mutants.iter().map(Mutant::describe_change).collect_vec();
        insta::assert_snapshot!(
            descriptions.join("\n"),
            @"replace controlled_loop with ()"
        );
    }

    #[test]
    fn mutate_factorial() {
        let source_file = SourceFile::new(
            Utf8Path::new("testdata/tree/factorial"),
            "src/bin/factorial.rs".parse().unwrap(),
            Rc::new("cargo-mutants-testdata-factorial".to_string()),
        )
        .unwrap();
        let mutants = discover_mutants(source_file.into()).unwrap();
        assert_eq!(mutants.len(), 2);

        let mut mutated_code = mutants[0].mutated_code();
        assert_eq!(mutants[0].function_name(), "main");
        mutated_code.retain(|c| c != '\r');
        assert_eq!(
            mutated_code,
            r#"fn main() {
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
        );

        let mut mutated_code = mutants[1].mutated_code();
        assert_eq!(mutants[1].function_name(), "factorial");
        mutated_code.retain(|c| c != '\r');
        assert_eq!(
            mutated_code,
            r#"fn main() {
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
        );
    }
}

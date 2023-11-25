// Copyright 2021 Martin Pool

//! Edit source code.

use serde::Serialize;

/// A (line, column) position in a source file.
#[derive(Clone, Copy, Eq, PartialEq, Debug, Serialize)]
pub struct LineColumn {
    /// 1-based line number.
    pub line: usize,

    /// 1-based column, measured in chars.
    pub column: usize,
}

impl From<proc_macro2::LineColumn> for LineColumn {
    fn from(l: proc_macro2::LineColumn) -> Self {
        LineColumn {
            line: l.line,
            column: l.column + 1,
        }
    }
}

/// A contiguous text span in a file.
///
/// TODO: Perhaps a semi-open range that can represent an empty span would be more general?
#[derive(Clone, Copy, Eq, PartialEq, Debug, Serialize)]
pub struct Span {
    /// The inclusive position where the span starts.
    pub start: LineColumn,
    /// The inclusive position where the span ends.
    pub end: LineColumn,
}

impl From<proc_macro2::Span> for Span {
    fn from(s: proc_macro2::Span) -> Self {
        Span {
            start: s.start().into(),
            end: s.end().into(),
        }
    }
}

impl From<&proc_macro2::Span> for Span {
    fn from(s: &proc_macro2::Span) -> Self {
        Span {
            start: s.start().into(),
            end: s.end().into(),
        }
    }
}

impl From<proc_macro2::extra::DelimSpan> for Span {
    fn from(s: proc_macro2::extra::DelimSpan) -> Self {
        // Get the span for the whole block from the start delimiter
        // to the end.
        let joined = s.join();
        Span {
            start: joined.start().into(),
            end: joined.end().into(),
        }
    }
}

/// Replace a subregion of text.
///
/// Returns a copy of `s` with the region between `start` and `end` inclusive replaced by
/// `replacement`.
pub(crate) fn replace_region(
    s: &str,
    start: &LineColumn,
    end: &LineColumn,
    replacement: &str,
) -> String {
    // dbg!(start, end);
    let mut r = String::with_capacity(s.len() + replacement.len());
    let mut line_no = 1;
    let mut col_no = 1;
    for c in s.chars() {
        if line_no < start.line
            || line_no > end.line
            || (line_no == start.line && col_no < start.column)
            || (line_no == end.line && col_no > end.column)
        {
            r.push(c);
        } else if line_no == start.line && col_no == start.column {
            r.push_str(replacement);
        }
        if c == '\n' {
            line_no += 1;
            col_no = 1;
        } else if c == '\r' {
            // counts as part of the last column, not a separate column
        } else {
            col_no += 1;
        }
    }
    r
}

#[cfg(test)]
mod test {
    use indoc::indoc;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn replace_treats_crlf_as_part_of_last_column() {
        let source = "fn foo() {\r\n    wibble();\r\n}\r\n//hey!\r\n";
        assert_eq!(
            replace_region(
                source,
                &LineColumn {
                    line: 1,
                    column: 10
                },
                &LineColumn { line: 3, column: 2 },
                "{}\r\n"
            ),
            "fn foo() {}\r\n//hey!\r\n"
        );
    }

    #[test]
    fn test_replace_region() {
        let source = indoc! { r#"

            fn foo() {
                some();
                stuff();
            }

            const BAR: u32 = 32;
        "# };
        // typical multi-line case
        let replaced = replace_region(
            source,
            &LineColumn {
                line: 2,
                column: 10,
            },
            &LineColumn { line: 5, column: 1 },
            "{ /* body deleted */ }",
        );
        assert_eq!(
            replaced,
            indoc! { r#"

                fn foo() { /* body deleted */ }

                const BAR: u32 = 32;
            "# }
        );

        // single-line case
        let replaced = replace_region(
            source,
            &LineColumn {
                line: 7,
                column: 18,
            },
            &LineColumn {
                line: 7,
                column: 19,
            },
            "69",
        );
        assert_eq!(
            replaced,
            indoc! { r#"

                fn foo() {
                    some();
                    stuff();
                }

                const BAR: u32 = 69;
            "# }
        );
    }
}

// Copyright 2021-2023 Martin Pool

//! Locations (line/column) and spans between them in source code.
//!
//! This is similar to, and can be automatically derived from,
//! [proc_macro2::Span] and [proc_macro2::LineColumn], but is
//! a bit more convenient for our purposes.

use std::fmt;

use serde::Serialize;

/// A (line, column) position in a source file.
#[derive(Clone, Copy, Eq, PartialEq, Serialize)]
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

impl fmt::Debug for LineColumn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LineColumn({}, {})", self.line, self.column)
    }
}

/// A contiguous text span in a file.
#[derive(Clone, Copy, Eq, PartialEq, Serialize)]
pub struct Span {
    /// The *inclusive* position where the span starts.
    pub start: LineColumn,
    /// The *exclusive* position where the span ends.
    pub end: LineColumn,
}

impl Span {
    #[allow(dead_code)]
    pub fn quad(
        start_line: usize,
        start_column: usize,
        end_line: usize,
        end_column: usize,
    ) -> Self {
        Span {
            start: LineColumn {
                line: start_line,
                column: start_column,
            },
            end: LineColumn {
                line: end_line,
                column: end_column,
            },
        }
    }

    /// Return the region of a multi-line string that this span covers.
    pub fn extract(&self, s: &str) -> String {
        let mut r = String::new();
        let mut line_no = 1;
        let mut col_no = 1;
        let start = self.start;
        let end = self.end;
        for c in s.chars() {
            if ((line_no == start.line && col_no >= start.column) || line_no > start.line)
                && (line_no < end.line || (line_no == end.line && col_no < end.column))
            {
                r.push(c);
            }
            if c == '\n' {
                line_no += 1;
                if line_no > end.line {
                    break;
                }
                col_no = 1;
            } else if c == '\r' {
                // counts as part of the last column, not a separate column
            } else {
                col_no += 1;
            }
            if line_no == end.line && col_no >= end.column {
                break;
            }
        }
        r
    }

    /// Replace a subregion of text.
    ///
    /// Returns a copy of `s` with the region identified by this span replaced by
    /// `replacement`.
    pub fn replace(&self, s: &str, replacement: &str) -> String {
        let mut r = String::with_capacity(s.len() + replacement.len());
        let mut line_no = 1;
        let mut col_no = 1;
        let start = self.start;
        let end = self.end;
        for c in s.chars() {
            if line_no == start.line && col_no == start.column {
                r.push_str(replacement);
            }
            if line_no < start.line
                || line_no > end.line
                || (line_no == start.line && col_no < start.column)
                || (line_no == end.line && col_no >= end.column)
            {
                r.push(c);
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
        if line_no == start.line && col_no == start.column {
            r.push_str(replacement);
        }
        r
    }
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

impl fmt::Debug for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // A concise form, similar to ::quad
        write!(
            f,
            "Span({}, {}, {}, {})",
            self.start.line, self.start.column, self.end.line, self.end.column
        )
    }
}

#[cfg(test)]
mod test {
    use indoc::indoc;
    // use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn linecolumn_debug_form() {
        let lc = LineColumn { line: 1, column: 2 };
        assert_eq!(format!("{:?}", lc), "LineColumn(1, 2)");
    }

    #[test]
    fn span_debug_form() {
        let span = Span::quad(1, 2, 3, 4);
        assert_eq!(format!("{:?}", span), "Span(1, 2, 3, 4)");
    }

    #[test]
    fn cut_before_crlf() {
        let source = "fn foo() {\r\n    wibble();\r\n}\r\n//hey!\r\n";
        let span = Span::quad(1, 10, 3, 2);
        assert_eq!(span.extract(source), "{\r\n    wibble();\r\n}");
        assert_eq!(span.replace(source, "{}"), "fn foo() {}\r\n//hey!\r\n");
    }

    #[test]
    fn empty_span_in_empty_string() {
        let span = Span::quad(1, 1, 1, 1);
        assert_eq!(span.extract(""), "");
        assert_eq!(span.replace("", "x"), "x");
    }

    #[test]
    fn empty_span_at_start_of_string() {
        let span = Span::quad(1, 1, 1, 1);
        assert_eq!(span.extract("hello"), "");
        assert_eq!(span.replace("hello", "x"), "xhello");
    }

    #[test]
    fn empty_span_at_end_of_string() {
        let span = Span::quad(1, 6, 1, 6);
        assert_eq!(span.extract("hello"), "");
        assert_eq!(span.replace("hello", "x"), "hellox");
    }

    #[test]
    fn cut_including_crlf() {
        let source = "fn foo() {\r\n    wibble();\r\n}\r\n//hey!\r\n";
        let span = Span::quad(1, 10, 3, 3);
        assert_eq!(span.extract(source), "{\r\n    wibble();\r\n}\r\n");
        assert_eq!(span.replace(source, "{}\r\n"), "fn foo() {}\r\n//hey!\r\n");
    }
    #[test]
    fn span_ops() {
        let source = indoc! { r#"

            fn foo() {
                some();
                stuff();
            }

            const BAR: u32 = 32;
        "# };
        // typical multi-line case
        let span = Span::quad(2, 10, 5, 2);
        assert_eq!(span.extract(source), "{\n    some();\n    stuff();\n}");
        let replaced = span.replace(source, "{ /* body deleted */ }");
        assert_eq!(
            replaced,
            indoc! { r#"

                fn foo() { /* body deleted */ }

                const BAR: u32 = 32;
            "# }
        );

        // single-line case
        let span = Span::quad(7, 18, 7, 20);
        assert_eq!(span.extract(source), "32");
        let replaced = span.replace(source, "69");
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

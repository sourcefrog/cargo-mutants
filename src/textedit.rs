// Copyright 2021 Martin Pool

use proc_macro2::LineColumn;

/// Return s with the specified inclusive region replaced.
///
/// In `LineColumn`, lines are 1-indexed, and inclusive; columns are 0-indexed
/// in UTF-8 characters (presumably really code points) and inclusive.
pub(crate) fn replace_region(
    s: &str,
    start: &LineColumn,
    end: &LineColumn,
    replacement: &str,
) -> String {
    dbg!(start, end);
    let mut r = String::with_capacity(s.len() + replacement.len());
    let mut line_no = 1;
    let mut col_no = 0;
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
            col_no = 0;
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
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn replace_treats_crlf_as_part_of_last_column() {
        let source = "fn foo() {\r\n    wibble();\r\n}\r\n//hey!\r\n";
        assert_eq!(
            replace_region(
                source,
                &LineColumn { line: 1, column: 9 },
                &LineColumn { line: 3, column: 1 },
                "{}\r\n"
            ),
            "fn foo() {}\r\n//hey!\r\n"
        );
    }

    #[test]
    fn test_replace_region() {
        let source = "
fn foo() {
    some();
    stuff();
}

const BAR: u32 = 32;
";
        // typical multi-line case
        assert_eq!(
            replace_region(
                &source,
                &LineColumn { line: 2, column: 9 },
                &LineColumn { line: 5, column: 0 },
                "{ /* body deleted */ }"
            ),
            "
fn foo() { /* body deleted */ }

const BAR: u32 = 32;
"
        );

        // single-line case
        assert_eq!(
            replace_region(
                &source,
                &LineColumn {
                    line: 7,
                    column: 17
                },
                &LineColumn {
                    line: 7,
                    column: 18
                },
                "69"
            ),
            "
fn foo() {
    some();
    stuff();
}

const BAR: u32 = 69;
"
        );
    }
}

// Copyright 2023-2024 Martin Pool

//! List mutants and files as text.

use std::fmt;
use std::io;

use serde_json::{json, Value};

use crate::mutate::Mutant;
use crate::path::Utf8PathSlashes;
use crate::source::SourceFile;
use crate::{Options, Result};

/// Convert `fmt::Write` to `io::Write`.
pub(crate) struct FmtToIoWrite<W: io::Write>(W);

impl<W: io::Write> FmtToIoWrite<W> {
    pub(crate) fn new(w: W) -> Self {
        Self(w)
    }
}

impl<W: io::Write> fmt::Write for FmtToIoWrite<W> {
    fn write_str(&mut self, s: &str) -> Result<(), fmt::Error> {
        self.0.write_all(s.as_bytes()).map_err(|_| fmt::Error)
    }
}

pub(crate) fn list_mutants<W: fmt::Write>(
    mut out: W,
    mutants: &[Mutant],
    options: &Options,
) -> Result<()> {
    if options.emit_json {
        let mut list: Vec<serde_json::Value> = Vec::new();
        for mutant in mutants {
            let mut obj = serde_json::to_value(mutant)?;
            if options.emit_diffs {
                obj.as_object_mut()
                    .unwrap()
                    .insert("diff".to_owned(), json!(mutant.diff()));
            }
            list.push(obj);
        }
        out.write_str(&serde_json::to_string_pretty(&list)?)?;
    } else {
        // TODO: Do we need to check this? Could the console library strip them if they're not
        // supported?
        let colors = options.colors.active_stdout();
        for mutant in mutants {
            writeln!(out, "{}", mutant.name(options.show_line_col, colors))?;
            if options.emit_diffs {
                writeln!(out, "{}", mutant.diff())?;
            }
        }
    }
    Ok(())
}

pub(crate) fn list_files<W: fmt::Write>(
    mut out: W,
    source_files: &[SourceFile],
    options: &Options,
) -> Result<()> {
    if options.emit_json {
        let json_list = Value::Array(
            source_files
                .iter()
                .map(|source_file| {
                    json!({
                        "path": source_file.tree_relative_path.to_slash_path(),
                        "package": source_file.package.name,
                    })
                })
                .collect(),
        );
        writeln!(out, "{}", serde_json::to_string_pretty(&json_list)?)?;
    } else {
        for file in source_files {
            writeln!(out, "{}", file.tree_relative_path.to_slash_path())?;
        }
    }
    Ok(())
}

// Copyright 2023 Martin Pool

//! List mutants and files as text.

use std::fmt;
use std::io;

use camino::Utf8Path;
use serde_json::{json, Value};

use crate::console::style_mutant;
use crate::path::Utf8PathSlashes;
use crate::{walk_tree, Options, Result, Tool};

/// Convert `fmt::Write` to `io::Write`.
pub(crate) struct GlueWrite<W: io::Write>(W);

impl<W: io::Write> GlueWrite<W> {
    pub(crate) fn new(w: W) -> Self {
        Self(w)
    }
}

impl<W: io::Write> fmt::Write for GlueWrite<W> {
    fn write_str(&mut self, s: &str) -> Result<(), fmt::Error> {
        self.0.write_all(s.as_bytes()).map_err(|_| fmt::Error)
    }
}

pub(crate) fn list_mutants<W: fmt::Write>(
    mut out: W,
    tool: &dyn Tool,
    source_tree_root: &Utf8Path,
    options: &Options,
) -> Result<()> {
    let discovered = walk_tree(tool, source_tree_root, options)?;
    if options.emit_json {
        let mut list: Vec<serde_json::Value> = Vec::new();
        for mutant in discovered.mutants {
            let mut obj = serde_json::to_value(&mutant)?;
            if options.emit_diffs {
                obj.as_object_mut()
                    .unwrap()
                    .insert("diff".to_owned(), json!(mutant.diff()));
            }
            list.push(obj);
        }
        out.write_str(&serde_json::to_string_pretty(&list)?)?;
    } else {
        for mutant in &discovered.mutants {
            if options.colors {
                writeln!(out, "{}", style_mutant(mutant))?;
            } else {
                writeln!(out, "{}", mutant)?;
            }
            if options.emit_diffs {
                writeln!(out, "{}", mutant.diff())?;
            }
        }
    }
    Ok(())
}

pub(crate) fn list_files<W: fmt::Write>(
    mut out: W,
    tool: &dyn Tool,
    source: &Utf8Path,
    options: &Options,
) -> Result<()> {
    let files = walk_tree(tool, source, options)?.files;
    if options.emit_json {
        let json_list = Value::Array(
            files
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
        for file in files {
            writeln!(out, "{}", file.tree_relative_path.to_slash_path())?;
        }
    }
    Ok(())
}

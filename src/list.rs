// Copyright 2023-2024 Martin Pool

//! List mutants and files as text or json.

#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

use itertools::Itertools;
use serde_json::{Value, json};

use crate::Options;
use crate::mutant::Mutant;
use crate::path::Utf8PathSlashes;
use crate::source::SourceFile;

/// Return a string representation of a list of mutants.
///
/// The format is controlled by the `emit_json`, `emit_diffs`, `show_line_col`, and `colors` options.
pub fn list_mutants(mutants: &[Mutant], options: &Options) -> String {
    if options.emit_json {
        // Panic: only if we created illegal json, which would be a bug.
        let mut list: Vec<serde_json::Value> = Vec::new();
        for mutant in mutants {
            let mut obj = serde_json::to_value(mutant).expect("Serialize mutant");
            if options.emit_diffs() {
                obj.as_object_mut().unwrap().insert(
                    "diff".to_owned(),
                    json!(mutant.diff(&mutant.mutated_code())),
                );
            }
            list.push(obj);
        }
        serde_json::to_string_pretty(&list).expect("Serialize mutants")
    } else {
        // TODO: Do we need to check this? Could the console library strip them if they're not
        // supported?
        let colors = options.colors.active_stdout();
        let mut out = String::with_capacity(200 * mutants.len());
        for mutant in mutants {
            if colors {
                out.push_str(&mutant.to_styled_string(options.show_line_col));
            } else {
                out.push_str(&mutant.name(options.show_line_col));
            }
            out.push('\n');
            if options.emit_diffs() {
                out.push_str(&mutant.diff(&mutant.mutated_code()));
                out.push('\n');
            }
        }
        out
    }
}

/// List the source files as json or text.
pub fn list_files(source_files: &[SourceFile], options: &Options) -> String {
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
        serde_json::to_string_pretty(&json_list).expect("Serialize source files")
    } else {
        source_files
            .iter()
            .map(|file| file.tree_relative_path.to_slash_path() + "\n")
            .join("")
    }
}

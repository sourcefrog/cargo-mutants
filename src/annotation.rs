// Copyright 2025 Martin Pool

//! Annotations: machine-readable messages that can e.g. cause mutants to be flagged in PRs.

use std::env;

use clap::ValueEnum;

use crate::mutant::Mutant;

/// Kind of error annotations to emit.
#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, ValueEnum)]
#[value(rename_all = "lower")]
pub enum AutoAnnotation {
    /// No annotations.
    None,
    /// Auto detect from the execution environment.
    #[default]
    Auto,
    /// GitHub annotations.
    GitHub,
}

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq)]
pub enum ResolvedAnnotation {
    /// No annotations.
    #[default]
    None,
    /// GitHub annotations.
    GitHub,
}

impl AutoAnnotation {
    /// Resolve the auto setting by auto-detecting CI environments.
    pub fn resolve(self) -> ResolvedAnnotation {
        match self {
            AutoAnnotation::Auto => {
                if env::var("GITHUB_ACTION").is_ok() {
                    ResolvedAnnotation::GitHub
                } else {
                    ResolvedAnnotation::None
                }
            }
            AutoAnnotation::GitHub => ResolvedAnnotation::GitHub,
            AutoAnnotation::None => ResolvedAnnotation::None,
        }
    }
}

impl ResolvedAnnotation {
    /// Format a message about this mutant being missed.
    pub fn format(self, mutant: &Mutant) -> String {
        match self {
            ResolvedAnnotation::None => String::new(),
            ResolvedAnnotation::GitHub => {
                // https://docs.github.com/en/actions/reference/workflow-commands-for-github-actions#setting-a-warning-message
                format!(
                    "::warning file={file},line={line},col={col},endLine={endline},endCol={endcol},title={title}:: {message}\n",
                    file = mutant.source_file.tree_relative_slashes(),
                    line = mutant.span.start.line,
                    col = mutant.span.start.column,
                    endline = mutant.span.end.line,
                    endcol = mutant.span.end.column,
                    message = mutant.describe_change(),
                    title = "Missed mutant",
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        options::Options,
        test_util::{single_threaded_remove_env_var, single_threaded_set_env_var},
        visit::mutate_source_str,
    };

    use super::*;

    use pretty_assertions::assert_eq;
    use rusty_fork::rusty_fork_test;

    rusty_fork_test! {
        #[test]
        fn resolve_auto_not_on_github() {
            single_threaded_remove_env_var("GITHUB_ACTION");
            assert_eq!(AutoAnnotation::Auto.resolve(), ResolvedAnnotation::None);
        }

        #[test]
        fn resolve_auto_github_isolated() {
            single_threaded_set_env_var("GITHUB_ACTION", "something");
            assert_eq!(AutoAnnotation::Auto.resolve(), ResolvedAnnotation::GitHub);
        }
    }

    #[test]
    fn resolve_simple() {
        assert_eq!(AutoAnnotation::None.resolve(), ResolvedAnnotation::None);
        assert_eq!(AutoAnnotation::GitHub.resolve(), ResolvedAnnotation::GitHub);
    }

    #[test]
    fn format_mutant_annotation() {
        let mutants = mutate_source_str("fn foo() { 1 + 2; }", &Options::default()).unwrap();
        let formatted = ResolvedAnnotation::GitHub.format(&mutants[0]);
        assert_eq!(
            formatted,
            "::warning file=src/main.rs,line=1,col=12,endLine=1,endCol=18,title=Missed mutant:: replace foo with ()\n"
        );
    }
}

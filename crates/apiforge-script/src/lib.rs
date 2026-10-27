//! ApiForge Script syntax, diagnostics and canonical source formatting.
pub mod ast;
pub mod complete;
mod format;
pub mod lexer;
mod lower;
mod parser;
pub use lower::{Document, LowerContext, Lowered, lower, validate_document_model};

pub use ast::{Diagnostic, Fix, Severity, SyntaxTree, UNATTACHED_COMMENT};
pub use format::{format, format_tree};
pub use parser::parse;

pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;
pub const MAX_DEPTH: usize = 64;
pub const MAX_DIAGNOSTICS: usize = 1000;

/// Adds an error to `diagnostics` within `MAX_DIAGNOSTICS`, counting what is
/// dropped in `omitted`. At the limit an error takes the place of the last
/// warning; with no warning left it takes the place of the last error when
/// `keep` is set (a project-level error found after the source's own, which
/// must never be the one dropped), and is dropped otherwise.
pub fn push_error(
    diagnostics: &mut Vec<Diagnostic>,
    omitted: &mut usize,
    error: Diagnostic,
    keep: bool,
) {
    if diagnostics.len() < MAX_DIAGNOSTICS {
        diagnostics.push(error);
        return;
    }
    *omitted += 1;
    let replaced = diagnostics
        .iter()
        .rposition(|d| d.severity == Severity::Warning)
        .or(keep.then(|| diagnostics.len() - 1));
    if let Some(index) = replaced {
        diagnostics.remove(index);
        diagnostics.push(error);
    }
}

mod reconcile;
pub use reconcile::{Reconciled, reconcile, semantic};

mod print;
pub use print::{print_component, print_folder_document, print_scenario};

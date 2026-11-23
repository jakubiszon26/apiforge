//! ApiForge Script syntax, diagnostics and canonical source formatting.
pub mod ast;
pub mod complete;
mod edit;
mod far;
mod fix;
mod format;
pub mod lexer;
mod lower;
mod parser;
pub use lower::{Document, LowerContext, Lowered, lower, validate_document_model};

pub use ast::{Diagnostic, Fix, Severity, SyntaxTree};
pub use edit::{edit_folder_document, edit_in_place, edit_in_place_moving};
pub use far::{FAR_READ, far_read_warnings};
pub use fix::{
    AFTER_REMOVED, Blocked, Blocker, Fixed, Judged, NOT_IN_PLACE, RUNS_NOWHERE, START_MISSING,
    START_NOT_FIRST, fix_source, judge_language_1,
};
pub use format::{format, format_tree};
pub use parser::parse;

/// The comments of `source` that are no statement's note, in source order
/// (LANG-06). Format keeps them; a file printed whole from its document, as
/// a board edit that cannot be written into the text or a publication
/// prints it, has no place for them, so the window and the MCP server ask
/// before such a print.
pub fn free_comments(source: &str) -> Vec<std::ops::Range<usize>> {
    parse(source)
        .comments
        .into_iter()
        .map(|comment| comment.range)
        .collect()
}

/// The `.afs` language this build reads and writes (LANG-00, section 2). It
/// is raised only when a file that was valid means something else or becomes
/// invalid; a project's manifest names the version its files are written in.
pub const LANGUAGE_VERSION: u32 = 2;
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

/// Adds `warnings` to `diagnostics` while the list has room under
/// `MAX_DIAGNOSTICS`, counting the rest in `omitted`: a pass run after
/// lowering (LAY-26's far reads) never takes an error's place.
pub fn push_warnings(
    diagnostics: &mut Vec<Diagnostic>,
    omitted: &mut usize,
    warnings: impl IntoIterator<Item = Diagnostic>,
) {
    for warning in warnings {
        if diagnostics.len() < MAX_DIAGNOSTICS {
            diagnostics.push(warning);
        } else {
            *omitted += 1;
        }
    }
}

/// The one-based line and column of a byte offset in `text`, the column
/// counted in characters: where the Code view and the CLI place a diagnostic.
/// An offset past the end or inside a character moves back to a boundary.
pub fn line_column(text: &str, offset: usize) -> (usize, usize) {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    let before = &text[..offset];
    let line = before.matches('\n').count() + 1;
    let column = before
        .rsplit('\n')
        .next()
        .map_or(0, |line| line.chars().count())
        + 1;
    (line, column)
}

mod reconcile;
pub use reconcile::{Reconciled, reconcile, semantic};

mod print;
pub use print::{
    binary_symbol, predicate_symbol, print_component, print_folder_document, print_revision,
    print_scenario, print_stored,
};

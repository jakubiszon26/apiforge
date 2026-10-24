//! ApiForge Script syntax, diagnostics and canonical source formatting.
pub mod ast;
pub mod complete;
mod format;
pub mod lexer;
mod lower;
mod parser;
pub use lower::{Document, LowerContext, Lowered, lower, validate_document_model};

pub use ast::{Diagnostic, Fix, Severity, SyntaxTree};
pub use format::{format, format_tree};
pub use parser::parse;

pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;
pub const MAX_DEPTH: usize = 64;
pub const MAX_DIAGNOSTICS: usize = 1000;

mod reconcile;
pub use reconcile::{Reconciled, reconcile, semantic};

mod print;
pub use print::{print_component, print_scenario};

//! ApiForge Script syntax, diagnostics and canonical source formatting.
pub mod ast;
mod format;
pub mod lexer;
mod parser;

pub use ast::{Diagnostic, Fix, Severity, SyntaxTree};
pub use format::{format, format_tree};
pub use parser::parse;

pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;
pub const MAX_DEPTH: usize = 64;
pub const MAX_DIAGNOSTICS: usize = 1000;

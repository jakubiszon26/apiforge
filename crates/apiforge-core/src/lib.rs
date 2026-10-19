//! GUI-independent project documents, validated imports and expression semantics.
//!
//! Parsing never runs requests. A document becomes available to callers only after
//! structural and graph validation; the v1 DTO stays intact for lossless round trips.
pub mod format;
pub mod js;
pub mod model;
pub mod operator;
pub mod path;
mod strict_json;
mod validation;

pub use format::{ImportError, ImportLimits, ValidatedDocument, import_document};

//! *Steps* (REQ-01a): a scenario read top to bottom as a numbered list, each
//! request with what it reads, what it sends that is special and what is
//! checked, and after a run what passed.
//!
//! [`model`] and [`sends`] say what the list shows, from the graphs alone and
//! without egui; [`list`] draws it and returns what a click asks for.

pub mod list;
pub mod model;
pub mod sends;

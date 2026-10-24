//! Editing operations of the board: connections, clipboard, component
//! extraction, response field pickers and project export.
//!
//! Every operation borrows its inputs and returns new values, so the editor can keep
//! the previous document as an undo snapshot. Rejected edits return an [`EditError`]
//! whose `Display` text is shown to the user as is.

mod clipboard;
mod component;
mod connect;
mod fold;
mod inspect;
mod nodes;
mod proxies;
mod storage;

pub use clipboard::{PASTE_OFFSET, Pasted, copy_selection, duplicate_selection, paste_selection};
pub use component::{
    ComponentDraft, DraftError, DraftSection, InputRow, InputTarget, OutputRow, add_component,
    initialize_inputs, initialize_outputs, materialize_component, validate_draft,
};
pub use connect::{connect, remove_connections};
pub use inspect::{
    RESPONSE_FIELD_DEPTH, RESPONSE_FIELD_LIMIT, ResponseField, ResponseFields,
    collect_response_fields, collect_response_fields_bounded, sample_paths,
};
pub use nodes::{
    AddedNode, AssignedVariable, OutputRef, add_node, assign_response_variable, field_assertion,
};
pub use proxies::{
    add_proxy, move_proxies, proxy_binding, proxy_source, prune_proxies, remove_proxies,
};
pub use storage::{migrate_demo_address, public_project};

pub(crate) use fold::push_identifier_part;

use crate::{ImportError, ids::new_id};
use thiserror::Error;

/// Why an edit was rejected. The messages are user-facing.
#[derive(Debug, Error)]
pub enum EditError {
    #[error("A block cannot be connected to itself.")]
    SelfConnection,
    #[error("Block not found.")]
    NodeNotFound,
    #[error("Start cannot have a predecessor.")]
    StartHasPredecessor,
    #[error("Choose an IF output: yes or no.")]
    BranchRequired,
    #[error("This connection already exists.")]
    DuplicateConnection,
    #[error("These ports have incompatible data types.")]
    IncompatiblePorts,
    #[error("This input already has a source. Remove its connection before assigning a new one.")]
    InputAlreadyBound,
    #[error("This connection would create a cycle. The test must have an unambiguous order.")]
    ConnectionCycle,
    #[error("The board already has a Start. Select blocks without Start to copy them.")]
    PasteSecondStart,
    #[error(
        "A data source of a copied block is missing. Restore it or copy the source block as well."
    )]
    PasteMissingSource,
    #[error("The scenario already has a Start block.")]
    SecondStart,
    #[error("The response source is no longer available.")]
    ResponseSourceUnavailable,
    #[error("Enter a variable name.")]
    VariableNameRequired,
    #[error("Choose an existing variable.")]
    VariableNotFound,
    #[error("Only a value marked as a variable can be placed on the board.")]
    NotAVariable,
    #[error(
        "This assignment creates a cycle: a request cannot use a variable taken from its own response."
    )]
    AssignmentCycle,
    #[error("{}", join_lines(.0))]
    InvalidDraft(Vec<DraftError>),
    #[error("Invalid operator argument path.")]
    InvalidOperandPath,
    #[error("The component to revise does not exist.")]
    ComponentNotFound,
    #[error("{0}")]
    InvalidProject(#[from] ImportError),
}

fn join_lines(errors: &[DraftError]) -> String {
    errors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

/// A fresh identifier that `taken` does not report as used.
fn unique_id(prefix: &str, taken: impl Fn(&str) -> bool) -> String {
    loop {
        let id = new_id(prefix);
        if !taken(&id) {
            return id;
        }
    }
}

#[cfg(test)]
mod test_support;

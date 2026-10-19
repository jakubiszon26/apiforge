//! Editing operations of the board: connections, clipboard, variables, component
//! extraction, response field pickers and project export.
//!
//! Every operation borrows its inputs and returns new values, so the editor can keep
//! the previous document as an undo snapshot. Rejected edits return an [`EditError`]
//! whose `Display` text is shown to the user as is.

mod clipboard;
mod connect;
mod fold;
mod nodes;
mod variables;

pub use clipboard::{PASTE_OFFSET, Pasted, copy_selection, duplicate_selection, paste_selection};
pub use connect::{connect, remove_connections};
pub use nodes::{
    AddedNode, AssignedVariable, OutputRef, add_node, assign_response_variable, field_assertion,
};
pub use variables::{
    TextSelection, VariableBlock, bind_variable_text, insert_variable_token, prune_template_inputs,
};

use crate::ids::new_id;
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
    #[error(
        "This assignment creates a cycle: a request cannot use a variable taken from its own response."
    )]
    AssignmentCycle,
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

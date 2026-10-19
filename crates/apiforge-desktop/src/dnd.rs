//! Drag-and-drop payloads shared by the library, the inspector and the board.
//!
//! Payloads describe what to create or bind, never runtime values, so dragging a
//! secret's name can never carry the secret itself.

use apiforge_core::{format::Binding, model::NodeKind, operator::Operator};

#[derive(Clone, Debug, PartialEq)]
pub enum Payload {
    /// A block from the library; components are pinned to a revision.
    Node {
        kind: NodeKind,
        component: Option<(String, u32)>,
    },
    /// An operator, added to the board as a formula node or into an argument slot.
    Operator(Operator),
    /// A typed source (variable, parameter, environment value or response field).
    Variable { label: String, binding: Binding },
}

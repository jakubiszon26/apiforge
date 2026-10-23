//! Blocks offered by the library and the add-block menu.
//!
//! A block is a node kind with a starting configuration. Several blocks create the
//! same kind: Value, Object and Variable are all Value nodes ([ADR 0004]).
//!
//! [ADR 0004]: ../../../docs/decisions/0004-document-format-v2.md

use crate::{
    format::{Binding, Component, Node, Position},
    graph::{VALUE_INPUT, create_node, create_value_node},
    model::NodeKind,
};
use indexmap::IndexMap;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Block {
    Start,
    Branch,
    Request,
    /// A value: data, a selected field or a calculation.
    Value,
    /// A Value building an object.
    Object,
    /// A Value marked as a variable.
    Variable,
    Comparison,
    Condition,
    Component,
    /// An output of the component being edited.
    Export,
}

impl Block {
    /// Built-in blocks in library order; components are listed separately.
    pub const BUILTIN: [Self; 7] = [
        Self::Start,
        Self::Branch,
        Self::Request,
        Self::Value,
        Self::Object,
        Self::Comparison,
        Self::Condition,
    ];

    pub fn kind(self) -> NodeKind {
        match self {
            Self::Start => NodeKind::Trigger,
            Self::Branch => NodeKind::If,
            Self::Request => NodeKind::Request,
            Self::Value | Self::Object | Self::Variable => NodeKind::Value,
            Self::Comparison => NodeKind::Assert,
            Self::Condition => NodeKind::Condition,
            Self::Component => NodeKind::Component,
            Self::Export => NodeKind::Export,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Value => "Value",
            Self::Object => "Object",
            Self::Variable => "Variable",
            other => other.kind().label(),
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Value => "Data, a selected field or a calculation",
            Self::Object => "Prepare an object from values",
            Self::Variable => "A value you can place anywhere",
            other => other.kind().description(),
        }
    }

    /// Whether a new block of this kind may be wired from a data output.
    pub fn accepts_data(self) -> bool {
        !matches!(self, Self::Start)
    }

    /// A new node of this block. A component block uses `component`'s revision.
    pub fn create(self, position: Position, component: Option<&Component>) -> Node {
        match self {
            Self::Object => {
                let fields = IndexMap::from([("field".to_owned(), Binding::literal(Value::Null))]);
                let mut node = create_value_node(Binding::Object { fields }, position);
                node.handle = "object".into();
                node
            }
            Self::Variable => {
                let mut node = create_node(NodeKind::Value, position, None);
                node.handle = "variable".into();
                node.config.variable = Some(true);
                node
            }
            other => create_node(other.kind(), position, component),
        }
    }

    /// The block a node was created as, as far as its configuration shows.
    pub fn of(node: &Node) -> Self {
        match node.kind {
            NodeKind::Trigger => Self::Start,
            NodeKind::If => Self::Branch,
            NodeKind::Request => Self::Request,
            NodeKind::Value if node.is_variable() => Self::Variable,
            NodeKind::Value => match node.inputs.get(VALUE_INPUT) {
                Some(Binding::Object { .. }) => Self::Object,
                _ => Self::Value,
            },
            NodeKind::Assert => Self::Comparison,
            NodeKind::Condition => Self::Condition,
            NodeKind::Component => Self::Component,
            NodeKind::Export => Self::Export,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AT: Position = Position { x: 0.0, y: 0.0 };

    #[test]
    fn blocks_create_nodes_they_recognise() {
        for block in Block::BUILTIN
            .into_iter()
            .chain([Block::Variable, Block::Export])
        {
            let node = block.create(AT, None);
            assert_eq!(node.kind, block.kind(), "{block:?}");
            assert_eq!(Block::of(&node), block, "{block:?}");
        }
        assert!(Block::Variable.create(AT, None).is_variable());
        assert_eq!(Block::Object.label(), "Object");
        assert_eq!(Block::Request.label(), "HTTP request");
    }
}

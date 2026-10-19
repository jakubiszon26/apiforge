//! Shared fixtures of the editing tests.
use crate::{
    format::{Environment, Graph, Node, Position, Project, Scenario},
    graph::create_node,
    model::{ErrorPolicy, NodeKind},
};
use indexmap::IndexMap;

/// A compatibility fixture project, e.g. `blocks.json`.
pub fn at(name: &str) -> Project {
    let bytes = match name {
        "blocks.json" => {
            include_bytes!("../../../../fixtures/compatibility/projects/blocks.json").as_slice()
        }
        "shop.json" => include_bytes!("../../../../fixtures/compatibility/projects/shop.json"),
        "tickets.json" => {
            include_bytes!("../../../../fixtures/compatibility/projects/tickets.json")
        }
        "workflow.json" => {
            include_bytes!("../../../../fixtures/compatibility/projects/workflow.json")
        }
        other => panic!("unknown fixture {other}"),
    };
    crate::import_document(bytes, &crate::ImportLimits::default())
        .expect("fixture imports")
        .into_workspace()
        .projects
        .remove(0)
}

/// A project with one empty scenario, like the reference `createEmptyProject`.
pub fn empty_project() -> Project {
    Project {
        version: 1,
        id: "empty".into(),
        name: "Empty".into(),
        scenarios: vec![Scenario {
            id: "scenario".into(),
            name: "Scenario".into(),
            description: String::new(),
            graph: Graph {
                nodes: vec![],
                edges: vec![],
            },
            cleanup: Graph {
                nodes: vec![],
                edges: vec![],
            },
            parameters: IndexMap::new(),
            error_policy: ErrorPolicy::Continue,
        }],
        components: vec![],
        contracts: vec![],
        environment: Environment {
            base_url: String::new(),
            variables: IndexMap::new(),
            secret_names: vec![],
        },
    }
}

/// A new node at the reference default position.
pub fn node(kind: NodeKind) -> Node {
    create_node(kind, Position { x: 100.0, y: 100.0 }, None)
}

/// A request, a formula and a comparison without connections.
pub struct Trio {
    pub graph: Graph,
    pub request: String,
    pub formula: String,
    pub assertion: String,
}

pub fn trio() -> Trio {
    let request = node(NodeKind::Request);
    let formula = node(NodeKind::Formula);
    let assertion = node(NodeKind::Assert);
    Trio {
        request: request.id.clone(),
        formula: formula.id.clone(),
        assertion: assertion.id.clone(),
        graph: Graph {
            nodes: vec![request, formula, assertion],
            edges: vec![],
        },
    }
}

//! Synthetic legacy JSON workspaces for migration tests (KTLO-03), built the
//! way the workspace window builds them: through the board's edit operations
//! and saved by the legacy `WorkspaceStore` writer, never through a folder
//! session. Reusable by anything that needs a realistic legacy workspace, such
//! as the legacy mode's removal (KTLO-06).
//!
//! Shared by path (`#[path]`), so it depends only on `apiforge-core` and
//! `apiforge-storage`.
#![allow(dead_code)]

use apiforge_core::{
    block::Block,
    demos,
    edit::{OutputRef, add_node, connect},
    format::{Binding, Graph, Position, Project, Workspace},
    graph::delete_nodes,
};
use apiforge_storage::WorkspaceStore;
use std::{collections::HashSet, path::Path};

/// The project the six-project workspace was refused on before migration
/// sealed its checkpoints in source order (KTLO-03, decision 6 of 2026-10-27).
pub const ORDER_DESK_ID: &str = "order-desk";
pub const ORDER_DESK_NAME: &str = "Order desk";
/// Its scenario, whose check stores its inputs out of source order.
pub const ORDER_TOTAL_ID: &str = "order-total";
pub const ORDER_TOTAL_NAME: &str = "Order total is checked";

/// The project migration still refuses: its scenario cannot be written as a
/// folder source.
pub const REFUSED_PROJECT_ID: &str = "partner-feed";
pub const REFUSED_PROJECT_NAME: &str = "Partner feed";
pub const REFUSED_SCENARIO_ID: &str = "feed-is-fetched";
pub const REFUSED_SCENARIO_NAME: &str = "Feed is fetched from an unset address";
/// The environment value its request reads, which the project never sets.
pub const UNSET_ENVIRONMENT_NAME: &str = "PARTNER_URL";

/// Six projects in the order the owner's workspace held them: the four demo
/// projects the workspace window offers, then two of the person's own. The
/// fifth, *Order desk*, holds the check that made migration refuse the owner's
/// workspace with four projects left in staging
/// (`docs/stage-ii-review-notes.md`); all six migrate now.
pub fn six_projects() -> Workspace {
    let mut projects = demos::demo_projects();
    projects.push(order_desk());
    projects.push(support_desk());
    Workspace {
        version: apiforge_core::format::FORMAT_VERSION,
        active_id: projects[0].id.clone(),
        projects,
    }
}

/// Two projects, the second deliberately one that migration refuses.
pub fn with_a_broken_project() -> Workspace {
    let projects = vec![support_desk(), partner_feed()];
    Workspace {
        version: apiforge_core::format::FORMAT_VERSION,
        active_id: projects[0].id.clone(),
        projects,
    }
}

/// Saves `workspace` as `workspace.json` in `directory` with the legacy writer
/// and returns that writer's exclusive session, ready to migrate.
pub fn write(directory: &Path, workspace: &Workspace) -> WorkspaceStore {
    let store = WorkspaceStore::open_exclusive(directory.join("workspace.json")).unwrap();
    store.save(workspace).unwrap();
    store
}

/// A check whose source was replaced on the board: the request it compared was
/// deleted, a new request was added, and its response was wired into the
/// check's `actual` input again. Deleting the request removed `actual`, and
/// wiring it back appended it after `expected`, so the legacy model holds the
/// inputs as `expected, actual`, the opposite of the order the printed source
/// gives them.
pub fn order_desk() -> Project {
    let mut project = demos::empty_project();
    project.id = ORDER_DESK_ID.into();
    project.name = ORDER_DESK_NAME.into();
    project.environment.base_url = "http://127.0.0.1:4318/demo/shop".into();
    let scenario = &mut project.scenarios[0];
    scenario.id = ORDER_TOTAL_ID.into();
    scenario.name = ORDER_TOTAL_NAME.into();
    let context = demos::empty_project();
    let mut graph = scenario.graph.clone();
    let start = graph.nodes[0].id.clone();
    let first = request(&mut graph, &start, "load_order", 140.0);
    let check = add_node(
        &graph,
        Block::Comparison,
        Position { x: 620.0, y: 140.0 },
        None,
        Some(OutputRef {
            node_id: &first,
            port: "status",
        }),
    )
    .unwrap();
    graph = check.graph;
    rename(&mut graph, &check.node_id, "total_is_paid");
    graph = delete_nodes(&graph, &HashSet::from([first.as_str()]));
    let second = request(&mut graph, &start, "reload_order", 300.0);
    graph = connect(
        &graph,
        &context,
        &second,
        "status",
        &check.node_id,
        "actual",
        &[],
    )
    .unwrap();
    scenario.graph = graph;
    project
}

/// An ordinary project of the person's own: one request after Start.
pub fn support_desk() -> Project {
    let mut project = demos::empty_project();
    project.id = "support-desk".into();
    project.name = "Support desk".into();
    let scenario = &mut project.scenarios[0];
    scenario.id = "ticket-lookup".into();
    scenario.name = "Ticket lookup".into();
    let start = scenario.graph.nodes[0].id.clone();
    request(&mut scenario.graph, &start, "load_ticket", 140.0);
    project
}

/// A request whose address is an environment value the project never sets.
/// A legacy document may keep such a name and the legacy writer saves it; a
/// folder source must name only values its environment holds, so the scenario
/// cannot be written as one and migration refuses the project.
pub fn partner_feed() -> Project {
    let mut project = demos::empty_project();
    project.id = REFUSED_PROJECT_ID.into();
    project.name = REFUSED_PROJECT_NAME.into();
    let scenario = &mut project.scenarios[0];
    scenario.id = REFUSED_SCENARIO_ID.into();
    scenario.name = REFUSED_SCENARIO_NAME.into();
    let start = scenario.graph.nodes[0].id.clone();
    let feed = request(&mut scenario.graph, &start, "load_feed", 140.0);
    scenario
        .graph
        .nodes
        .iter_mut()
        .find(|node| node.id == feed)
        .unwrap()
        .inputs
        .insert(
            "url".into(),
            Binding::Environment {
                name: UNSET_ENVIRONMENT_NAME.into(),
            },
        );
    project
}

fn request(graph: &mut Graph, after: &str, handle: &str, y: f64) -> String {
    let added = add_node(
        graph,
        Block::Request,
        Position { x: 320.0, y },
        None,
        Some(OutputRef {
            node_id: after,
            port: "__order",
        }),
    )
    .unwrap();
    *graph = added.graph;
    rename(graph, &added.node_id, handle);
    added.node_id
}

fn rename(graph: &mut Graph, id: &str, handle: &str) {
    graph
        .nodes
        .iter_mut()
        .find(|node| node.id == id)
        .unwrap()
        .handle = handle.into();
}

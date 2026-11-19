//! LANG-13 (#386): a folder project in the written language — publication
//! keeps the stored spelling, board edits keep the chain, and a model written
//! as a source for the first time is written as a chain.
use apiforge_core::{
    ImportLimits,
    block::Block,
    edit::{OutputRef, add_node, connect, remove_connections},
    format::{Graph, Position},
    graph::{ORDER_PORT, TRUE_PORT, delete_nodes, sequence},
    import_document,
    model::NodeKind,
};
use apiforge_project::*;
use apiforge_script::{Diagnostic, LowerContext, Severity, lower, parse, semantic};
use apiforge_storage::WorkspaceStore;
use std::{collections::HashSet, path::Path};

fn errors(diagnostics: &[Diagnostic]) -> Vec<&Diagnostic> {
    diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect()
}

fn main_graph(document: &Document) -> &Graph {
    match document {
        Document::Scenario(scenario) => &scenario.graph,
        Document::Component(component) => &component.graph,
    }
}

fn main_graph_mut(document: &mut Document) -> &mut Graph {
    match document {
        Document::Scenario(scenario) => &mut scenario.graph,
        Document::Component(component) => &mut component.graph,
    }
}

/// The handles of the steps in the order the engine reaches them.
fn steps(graph: &Graph) -> Vec<String> {
    sequence(graph)
        .into_iter()
        .map(|id| graph.nodes.iter().find(|node| node.id == id).unwrap())
        .filter(|node| !node.kind.is_data())
        .map(|node| node.handle.clone())
        .collect()
}

fn id_of(document: &Document, handle: &str) -> String {
    main_graph(document)
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("no block `{handle}`"))
        .id
        .clone()
}

fn prepared_demo() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    prepare_project(directory.path(), &apiforge_core::demos::shop_project()).unwrap();
    directory
}

/// Every document of the session analysed, none with an error, none with
/// `@after` in its text.
fn every_document_passes(session: &mut ProjectSession) {
    session.settle_all().unwrap();
    for document in session.documents() {
        let path = document.entry().path.as_str().to_owned();
        assert!(
            errors(document.diagnostics()).is_empty(),
            "{path}\n{}\n{:#?}",
            document.text(),
            document.diagnostics()
        );
        assert!(document.model().is_some(), "{path}");
        assert!(!document.text().contains("@after"), "{path}");
    }
}

// Criterion 8 ----------------------------------------------------------------

const PAIR: &str = "component pair \"Pair\" {\n  begin = start\n  b = GET \"/b\" {\n    sample 200 { id: 1 }\n  }\n  a = GET \"/a\" {\n    sample 200 { id: 2 }\n  }\n  export id: number = a.body.id\n}\n";

#[test]
fn lang13_criterion_8_publication_keeps_the_stored_spelling() {
    let directory = prepared_demo();
    let mut session = ProjectSession::open(directory.path()).unwrap();
    let id = session
        .create_document(DocumentKind::Component, "components/pair.afs", PAIR)
        .unwrap();
    let source = session.document(&id).unwrap().model().unwrap().clone();
    let published = session.publish(&id, Origin(1)).unwrap();
    let path = directory.path().join(format!(
        ".apiforge/revisions/{}/{}.afs",
        published.component_id, published.revision
    ));
    let revision = std::fs::read_to_string(path).unwrap();
    // The revision is read as 0.1.0 reads it, and says what the source says.
    let context = session.context().clone();
    let lowered = lower(&parse(&revision), &LowerContext::folder(&context).stored());
    assert!(
        !lowered.has_errors(),
        "{revision}\n{:#?}",
        lowered.diagnostics
    );
    assert_eq!(
        semantic(&lowered.document.unwrap(), &context),
        semantic(&source, &context)
    );
    // The loader reads it back the same way.
    drop(session);
    let session = ProjectSession::open(directory.path()).unwrap();
    let loaded = session
        .context()
        .components
        .iter()
        .find(|c| c.id == published.component_id && c.revision == published.revision)
        .expect("the revision is loaded")
        .clone();
    assert_eq!(steps(&loaded.graph), ["begin", "b", "a"]);
}

// Criterion 9 ----------------------------------------------------------------

/// A chain of three requests, a check of the middle one and a branch.
const CHAIN: &str = r#"scenario "Chain" {
  begin = start
  first = GET "/first" {
    sample 200 { id: 1 }
  }
  middle = GET "/middle" {
    sample 200 { id: 2 }
  }
  middle_ok = assert middle.status == 200
  last = GET "/last" {
    sample 200 { id: 3 }
  }
  chosen = if last.status == 200 {
    inside = GET "/inside" {
      sample 200 {}
    }
  }
}
"#;

fn opened() -> (tempfile::TempDir, ProjectSession, String) {
    let directory = prepared_demo();
    let mut session = ProjectSession::open(directory.path()).unwrap();
    let id = session
        .create_document(DocumentKind::Scenario, "scenarios/chain.afs", CHAIN)
        .unwrap();
    assert!(session.document(&id).unwrap().model().is_some());
    (directory, session, id)
}

fn model(session: &ProjectSession, id: &str) -> Document {
    session.document(id).unwrap().model().unwrap().clone()
}

fn text(session: &ProjectSession, id: &str) -> String {
    session.document(id).unwrap().text().to_owned()
}

/// The edited text passes the check, holds no `@after` and reads back as the
/// session's model.
fn passes(session: &mut ProjectSession, id: &str) -> Document {
    let written = text(session, id);
    assert!(!written.contains("@after"), "{written}");
    let context = session.context().clone();
    let lowered = lower(&parse(&written), &LowerContext::folder(&context));
    assert!(
        errors(&lowered.diagnostics).is_empty(),
        "{written}\n{:#?}",
        lowered.diagnostics
    );
    let current = model(session, id);
    assert_eq!(
        semantic(lowered.document.as_ref().unwrap(), &context),
        semantic(&current, &context),
        "{written}"
    );
    current
}

fn add(document: &Document, block: Block, source: Option<(&str, &str)>) -> (Document, String) {
    let mut next = document.clone();
    let graph = main_graph_mut(&mut next);
    let added = add_node(
        graph,
        block,
        Position { x: 40.0, y: 40.0 },
        None,
        source.map(|(node_id, port)| OutputRef { node_id, port }),
    )
    .expect("the block is added");
    *graph = added.graph;
    let handle = graph
        .nodes
        .iter()
        .find(|node| node.id == added.node_id)
        .unwrap()
        .handle
        .clone();
    (next, handle)
}

#[test]
fn lang13_criterion_9_a_block_from_the_library_runs_last() {
    let (_directory, mut session, id) = opened();
    let (edited, handle) = add(&model(&session, &id), Block::Request, None);
    session.apply_model_edit(&id, edited, Origin(1)).unwrap();
    let after = passes(&mut session, &id);
    let order = steps(main_graph(&after));
    assert_eq!(order.last(), Some(&handle), "{order:?}");
}

#[test]
fn lang13_criterion_9_a_step_from_next_of_a_middle_step_runs_between() {
    let (_directory, mut session, id) = opened();
    let before = model(&session, &id);
    let middle = id_of(&before, "middle");
    let (edited, handle) = add(&before, Block::Request, Some((&middle, ORDER_PORT)));
    session.apply_model_edit(&id, edited, Origin(1)).unwrap();
    let after = passes(&mut session, &id);
    assert_eq!(
        steps(main_graph(&after)),
        [
            "begin",
            "first",
            "middle",
            handle.as_str(),
            "last",
            "chosen",
            "inside"
        ]
    );
}

#[test]
fn lang13_criterion_9_a_step_from_a_branch_port_runs_in_the_branch() {
    let (_directory, mut session, id) = opened();
    let before = model(&session, &id);
    let chosen = id_of(&before, "chosen");
    let (edited, handle) = add(&before, Block::Request, Some((&chosen, TRUE_PORT)));
    session.apply_model_edit(&id, edited, Origin(1)).unwrap();
    let after = passes(&mut session, &id);
    assert_eq!(
        steps(main_graph(&after)),
        [
            "begin",
            "first",
            "middle",
            "last",
            "chosen",
            "inside",
            handle.as_str()
        ]
    );
    let written = text(&session, &id);
    let inside = written.find("inside = GET").unwrap();
    let new = written.find(&format!("{handle} = GET")).unwrap();
    let close = written[inside..].find("\n  }\n").unwrap() + inside;
    assert!(inside < new && new < close, "{written}");
}

#[test]
fn lang13_criterion_9_a_removed_middle_step_joins_its_neighbours() {
    let (_directory, mut session, id) = opened();
    let before = model(&session, &id);
    let middle = id_of(&before, "middle");
    let check = id_of(&before, "middle_ok");
    let mut edited = before.clone();
    let graph = main_graph_mut(&mut edited);
    *graph = delete_nodes(graph, &HashSet::from([middle.as_str(), check.as_str()]));
    session.apply_model_edit(&id, edited, Origin(1)).unwrap();
    let after = passes(&mut session, &id);
    assert_eq!(
        steps(main_graph(&after)),
        ["begin", "first", "last", "chosen", "inside"]
    );
}

#[test]
fn lang13_criterion_9_an_ordering_wire_between_existing_steps_changes_nothing() {
    let (_directory, mut session, id) = opened();
    let before = model(&session, &id);
    let written = text(&session, &id);
    let context = session.context().clone();
    // A wire from `first` to `last`.
    let wired = {
        let mut next = before.clone();
        let graph = main_graph_mut(&mut next);
        *graph = connect(
            graph,
            &context,
            &id_of(&before, "first"),
            ORDER_PORT,
            &id_of(&before, "last"),
            ORDER_PORT,
            &[],
        )
        .expect("the wire is drawn");
        next
    };
    session.apply_model_edit(&id, wired, Origin(1)).unwrap();
    assert_eq!(text(&session, &id), written);
    assert_eq!(
        semantic(&model(&session, &id), &context),
        semantic(&before, &context)
    );
    // Deleting the wire from `middle` to `last`.
    let unwired = {
        let mut next = before.clone();
        let graph = main_graph_mut(&mut next);
        let (middle, last) = (id_of(&before, "middle"), id_of(&before, "last"));
        let wire = graph
            .edges
            .iter()
            .find(|edge| edge.source == middle && edge.target == last)
            .expect("middle is followed by last")
            .id
            .clone();
        *graph = remove_connections(graph, &HashSet::from([wire.as_str()]));
        next
    };
    session.apply_model_edit(&id, unwired, Origin(2)).unwrap();
    assert_eq!(text(&session, &id), written);
    assert_eq!(
        semantic(&model(&session, &id), &context),
        semantic(&before, &context)
    );
}

// Criterion 10 ---------------------------------------------------------------

#[test]
fn lang13_criterion_10_a_prepared_demo_is_written_as_a_chain() {
    let shop = apiforge_core::demos::shop_project();
    let directory = tempfile::tempdir().unwrap();
    prepare_project(directory.path(), &shop).unwrap();
    let mut session = ProjectSession::open(directory.path()).unwrap();
    every_document_passes(&mut session);
    for scenario in &shop.scenarios {
        let document = session
            .documents()
            .iter()
            .find(|d| d.entry().entity_id == scenario.id)
            .expect("the scenario is written");
        let model = document.model().unwrap();
        let without_start = |graph: &Graph| -> Vec<String> {
            steps(graph)
                .into_iter()
                .filter(|handle| {
                    graph
                        .nodes
                        .iter()
                        .any(|node| node.handle == *handle && node.kind != NodeKind::Start)
                })
                .collect()
        };
        assert_eq!(
            without_start(main_graph(model)),
            without_start(&scenario.graph),
            "{}",
            document.text()
        );
        let Document::Scenario(written) = model else {
            panic!("a scenario");
        };
        assert_eq!(
            without_start(&written.cleanup),
            without_start(&scenario.cleanup)
        );
    }
}

#[test]
fn lang13_criterion_10_the_legacy_workspace_migrates_and_every_project_passes() {
    let directory = tempfile::tempdir().unwrap();
    let workspace = import_document(
        include_bytes!("../../../fixtures/workspace.json"),
        &ImportLimits::default(),
    )
    .unwrap()
    .into_workspace();
    let store = WorkspaceStore::open_exclusive(directory.path().join("workspace.json")).unwrap();
    store.save(&workspace).unwrap();
    let outcome = migrate_workspace(&store).unwrap();
    assert_eq!(outcome.catalogue.projects.len(), workspace.projects.len());
    for entry in &outcome.catalogue.projects {
        let mut session = ProjectSession::open(Path::new(&entry.path)).unwrap();
        every_document_passes(&mut session);
    }
}

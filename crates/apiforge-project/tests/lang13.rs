//! LANG-13 (#386): a folder project in the written language — publication
//! keeps the stored spelling, board edits keep the chain, and a model written
//! as a source for the first time is written as a chain.
use apiforge_core::{
    ImportLimits,
    block::Block,
    edit::{OutputRef, Place, add_node, place},
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

const PAIR: &str = "component pair \"Pair\" {\n  b = GET \"/b\" {\n    sample 200 { id: 1 }\n  }\n  a = GET \"/a\" {\n    sample 200 { id: 2 }\n  }\n  export id: number = a.body.id\n}\n";

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
    assert_eq!(steps(&loaded.graph), ["b", "a"]);
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

/// Restated by DIAG-06a (#391), which replaces LANG-13's interim: an
/// ordering wire between two existing steps no longer changes nothing, it
/// moves the step it is dropped on to directly after the step it comes from
/// (and its check with it), and only their lines move in the text.
#[test]
fn lang13_criterion_9_an_ordering_wire_between_existing_steps_moves_the_step() {
    let (_directory, mut session, id) = opened();
    let before = model(&session, &id);
    let context = session.context().clone();
    // A wire from `first`'s *Next* to `last`.
    let (graph, moved) = place(
        main_graph(&before),
        &context,
        &id_of(&before, "last"),
        Place::After(&id_of(&before, "first")),
    )
    .expect("the move is allowed");
    let mut next = before.clone();
    *main_graph_mut(&mut next) = graph;
    session.apply_move(&id, next, &moved, Origin(1)).unwrap();
    let after = passes(&mut session, &id);
    assert_eq!(
        steps(main_graph(&after)),
        ["begin", "first", "last", "middle", "chosen", "inside"]
    );
    assert_eq!(
        text(&session, &id),
        CHAIN
            .replace(
                "  last = GET \"/last\" {\n    sample 200 { id: 3 }\n  }\n",
                ""
            )
            .replace(
                "  middle = GET",
                "  last = GET \"/last\" {\n    sample 200 { id: 3 }\n  }\n  middle = GET"
            )
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

// Criterion 15 ---------------------------------------------------------------

#[path = "support/language.rs"]
mod language;

/// A5: in a project still in language 1 the `@after` and `start` errors are
/// shown without their fix (the unit of conversion is the project, LANG-20),
/// and a run is refused with LANG-15's sentence; in a language-2 project the
/// same text offers the fix. Restated by LANG-20 (#481, criterion 10): in
/// language 1 the error on the `@after` line names `apiforge fix --folder`
/// instead of saying "delete this line".
#[test]
fn lang13_criterion_15_no_fix_in_an_older_project() {
    use apiforge_script::{AFTER_REMOVED, START_MISSING};
    assert_eq!(apiforge_script::LANGUAGE_VERSION, 2);
    // Written for 0.1.0: no `start` and an `@after`.
    let text = "scenario \"Old\" {\n  first = GET \"/first\" {\n    sample 200 {}\n  }\n  @after(first)\n  second = GET \"/second\" {\n    sample 200 {}\n  }\n}\n";
    let fixes = |root: &Path| {
        let older = apiforge_project::language::after_in_older(root);
        let mut session = ProjectSession::open(root).unwrap();
        let id = session
            .documents()
            .iter()
            .find(|d| d.entry().path.as_str() == "scenarios/old.afs")
            .unwrap()
            .id()
            .to_owned();
        session.settle(&id).unwrap();
        let ours: Vec<(bool, bool, bool)> = session
            .document(&id)
            .unwrap()
            .diagnostics()
            .iter()
            .filter(|d| {
                matches!(d.message.as_str(), AFTER_REMOVED | START_MISSING) || d.message == older
            })
            .map(|d| {
                (
                    d.message == AFTER_REMOVED,
                    d.message == older,
                    d.fix.is_some(),
                )
            })
            .collect();
        let run = session.prepare_run(&id).map(|_| ()).err();
        (ours, run)
    };
    let directory = prepared_demo();
    let root = directory.path();
    let mut session = ProjectSession::open(root).unwrap();
    session
        .create_document(DocumentKind::Scenario, "scenarios/old.afs", text)
        .unwrap();
    drop(session);

    // Language 2: both errors carry their fix; the `@after` one says
    // "delete this line".
    let (ours, _) = fixes(root);
    assert_eq!(ours.len(), 2, "{ours:?}");
    assert!(ours.iter().all(|(_, _, fix)| *fix), "{ours:?}");
    assert_eq!(ours.iter().filter(|(after, ..)| *after).count(), 1);

    // Language 1: the same errors, without a fix, the `@after` one naming
    // the command; a run is refused.
    language::write_language(root, 1);
    let (ours, run) = fixes(root);
    assert_eq!(ours.len(), 2, "{ours:?}");
    assert!(ours.iter().all(|(_, _, fix)| !*fix), "{ours:?}");
    assert_eq!(ours.iter().filter(|(after, ..)| *after).count(), 0);
    assert_eq!(ours.iter().filter(|(_, older, _)| *older).count(), 1);
    let run = run.expect("a run is refused");
    assert!(matches!(run, Error::OlderLanguage { .. }), "{run:?}");
    assert!(
        run.to_string()
            .starts_with("This project is written in language 1; this build reads language 2."),
        "{run}"
    );
}

// Review of #488 -------------------------------------------------------------

/// The start a board edit writes into a scenario takes a handle and an
/// identifier no block of the file has, cleanup included.
#[test]
fn lang13_review_the_written_start_avoids_the_cleanups_handles_and_ids() {
    let directory = prepared_demo();
    let mut session = ProjectSession::open(directory.path()).unwrap();
    let id = session
        .create_document(
            DocumentKind::Scenario,
            "scenarios/x.afs",
            "scenario \"X\" {\n  cleanup {\n    begin = DELETE \"/temporary\"\n  }\n}\n",
        )
        .unwrap();
    let mut before = model(&session, &id);
    // The cleanup block's persistent identifier is `start`.
    if let apiforge_script::Document::Scenario(scenario) = &mut before {
        scenario.cleanup.nodes[0].id = "start".into();
    }
    let (edited, _) = add(&before, Block::Request, None);
    session.apply_model_edit(&id, edited, Origin(1)).unwrap();
    let after = passes(&mut session, &id);
    let apiforge_script::Document::Scenario(scenario) = &after else {
        panic!("a scenario");
    };
    let start = scenario
        .graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Start)
        .expect("a start is written");
    assert_eq!(start.handle, "begin_2");
    assert!(
        scenario
            .cleanup
            .nodes
            .iter()
            .all(|node| node.id != start.id)
    );
}

/// The same when a legacy scenario without a start is migrated: its cleanup
/// already has a block `begin` whose identifier is `start`.
#[test]
fn lang13_review_migration_writes_a_start_beside_a_cleanup_begin() {
    let mut project = apiforge_core::demos::shop_project();
    let scenario = &mut project.scenarios[0];
    scenario
        .graph
        .nodes
        .retain(|node| node.kind != NodeKind::Start);
    // A cleanup block no other block reads or follows.
    let begin = scenario
        .cleanup
        .nodes
        .iter_mut()
        .find(|node| node.kind == NodeKind::Assert)
        .expect("the demo's cleanup has a check");
    begin.id = "start".into();
    begin.handle = "begin".into();

    let written = as_written(&apiforge_script::Document::Scenario(
        project.scenarios[0].clone(),
    ))
    .unwrap();
    let apiforge_script::Document::Scenario(written) = written else {
        panic!("a scenario");
    };
    let start = written
        .graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Start)
        .expect("a start is written");
    assert_eq!(start.handle, "begin_2");
    assert_ne!(start.id, "start");

    let directory = tempfile::tempdir().unwrap();
    let store = WorkspaceStore::open_exclusive(directory.path().join("workspace.json")).unwrap();
    store
        .save(&apiforge_core::format::Workspace {
            version: project.version,
            active_id: project.id.clone(),
            projects: vec![project],
        })
        .unwrap();
    let outcome = migrate_workspace(&store).expect("the project migrates");
    let mut session = ProjectSession::open(Path::new(&outcome.catalogue.projects[0].path)).unwrap();
    every_document_passes(&mut session);
}

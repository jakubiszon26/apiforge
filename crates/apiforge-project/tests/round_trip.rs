//! The P2 gate: a folder project written, edited in code and on the diagram,
//! saved, closed and reopened keeps semantics, identities, layout, proxies and
//! exact component revisions, including cleanup graphs.
use apiforge_core::{
    ImportLimits,
    format::{Position, Project, ProxySource},
    import_document,
};
use apiforge_project::*;

fn fixtures() -> Vec<Project> {
    import_document(
        include_bytes!("../../../fixtures/workspace.json"),
        &ImportLimits::default(),
    )
    .unwrap()
    .into_workspace()
    .projects
}

/// The shop fixture with a variable proxy, an environment proxy and a second
/// published revision that its scenario does not use.
fn shop() -> Project {
    let mut project = fixtures().into_iter().find(|p| p.id == "shop").unwrap();
    project
        .environment
        .variables
        .insert("region".into(), "eu".into());
    let scenario = &mut project.scenarios[0];
    assert!(!scenario.cleanup.nodes.is_empty(), "covers cleanup");
    scenario.graph = apiforge_core::edit::add_proxy(
        &scenario.graph,
        ProxySource::Environment {
            name: "region".into(),
        },
        Position { x: -320.0, y: 48.0 },
    )
    .unwrap();
    let value = scenario
        .graph
        .nodes
        .iter_mut()
        .find(|node| node.kind == apiforge_core::model::NodeKind::Value)
        .unwrap();
    value.config.variable = Some(true);
    let variable = value.id.clone();
    scenario.graph = apiforge_core::edit::add_proxy(
        &scenario.graph,
        ProxySource::Variable { node_id: variable },
        Position { x: 640.0, y: -96.0 },
    )
    .unwrap();
    assert_eq!(scenario.graph.proxies.len(), 2);
    let mut newer = project.components[0].clone();
    newer.revision = 2;
    newer.name.push_str(" v2");
    project.components.push(newer);
    project
}

fn scenario_document(session: &ProjectSession, id: &str) -> String {
    session
        .documents()
        .iter()
        .find(|document| {
            document.entry().entity_id == id && document.entry().kind == DocumentKind::Scenario
        })
        .unwrap()
        .id()
        .to_owned()
}

fn model(session: &ProjectSession, document_id: &str) -> Document {
    session
        .document(document_id)
        .unwrap()
        .model()
        .unwrap()
        .clone()
}

#[test]
fn every_fixture_reopens_with_identical_models_and_published_revisions() {
    for project in fixtures().into_iter().chain([shop()]) {
        let directory = tempfile::tempdir().unwrap();
        prepare_project(directory.path(), &project).unwrap();
        let session = ProjectSession::open(directory.path()).unwrap();
        assert!(!session.is_read_only());
        for scenario in &project.scenarios {
            let id = scenario_document(&session, &scenario.id);
            let document = session.document(&id).unwrap();
            assert_eq!(
                document.model_state(),
                ModelState::Current,
                "{}",
                project.id
            );
            assert!(!document.is_dirty());
            assert_eq!(model(&session, &id), Document::Scenario(scenario.clone()));
        }
        let mut published = session.context().components.clone();
        let mut expected = project.components.clone();
        published.sort_by(|a, b| (&a.id, a.revision).cmp(&(&b.id, b.revision)));
        expected.sort_by(|a, b| (&a.id, a.revision).cmp(&(&b.id, b.revision)));
        assert_eq!(published, expected, "{}", project.id);
        for document in session.documents() {
            if document.entry().kind != DocumentKind::Component {
                continue;
            }
            let Some(Document::Component(working)) = document.model() else {
                panic!("working component model");
            };
            let latest = expected
                .iter()
                .filter(|component| component.id == working.id)
                .max_by_key(|component| component.revision)
                .unwrap();
            assert_eq!(document.entry().base_revision, Some(latest.revision));
            assert_eq!(working.revision, latest.revision + 1);
            let mut base = latest.clone();
            base.revision = working.revision;
            assert_eq!(working, &base, "identity and layout of the working copy");
        }
    }
}

#[test]
fn diagram_and_code_edits_survive_save_close_and_reopen() {
    let project = shop();
    let directory = tempfile::tempdir().unwrap();
    prepare_project(directory.path(), &project).unwrap();
    let original = &project.scenarios[0];
    let mut session = ProjectSession::open(directory.path()).unwrap();
    let id = scenario_document(&session, &original.id);
    let view = Origin(7);

    // Moving a block changes layout only: the source stays clean.
    let Document::Scenario(mut moved) = model(&session, &id) else {
        panic!("scenario")
    };
    moved.graph.nodes[0].position = Position {
        x: 1234.0,
        y: 567.0,
    };
    moved.graph.nodes[0].auto_placed = false;
    session
        .apply_model_edit(&id, Document::Scenario(moved.clone()), view)
        .unwrap();
    let document = session.document(&id).unwrap();
    assert!(!document.is_dirty() && document.is_layout_dirty());

    // A semantic graphical edit reprints canonical source.
    moved.description = "Edited on the diagram".into();
    session
        .apply_model_edit(&id, Document::Scenario(moved.clone()), view)
        .unwrap();
    assert!(session.document(&id).unwrap().is_dirty());
    assert!(
        session
            .document(&id)
            .unwrap()
            .text()
            .contains("Edited on the diagram")
    );

    // A code edit of one statement keeps every other identity and position.
    let text = session.document(&id).unwrap().text().to_owned();
    let edited = text.replacen("Edited on the diagram", "Edited in code", 1);
    session.edit_source(&id, &edited, view).unwrap();
    assert_eq!(
        session.document(&id).unwrap().model_state(),
        ModelState::LastValid
    );
    session.settle(&id).unwrap();
    let Document::Scenario(expected) = model(&session, &id) else {
        panic!("scenario")
    };
    assert_eq!(expected.description, "Edited in code");
    assert_eq!(
        expected.graph.nodes[0].position,
        moved.graph.nodes[0].position
    );
    assert_eq!(expected.graph.proxies, original.graph.proxies);
    assert_eq!(expected.cleanup, original.cleanup);

    let saved = session.save(&id).unwrap();
    assert!(saved.checkpoint && saved.generation.is_some());
    assert!(!session.document(&id).unwrap().is_dirty());
    let saved_text = session.document(&id).unwrap().text().to_owned();
    drop(session);

    let session = ProjectSession::open(directory.path()).unwrap();
    let document = session.document(&id).unwrap();
    assert_eq!(document.text(), saved_text);
    assert!(!document.is_dirty());
    assert_eq!(model(&session, &id), Document::Scenario(expected.clone()));
    for (node, before) in expected.graph.nodes.iter().zip(&original.graph.nodes) {
        assert_eq!(node.id, before.id);
    }
    // The scenario still calls exactly the revision it was written against.
    let revisions: Vec<_> = expected
        .graph
        .nodes
        .iter()
        .filter_map(|node| node.config.revision)
        .collect();
    let before: Vec<_> = original
        .graph
        .nodes
        .iter()
        .filter_map(|node| node.config.revision)
        .collect();
    assert!(!before.is_empty());
    assert_eq!(revisions, before);
}

#[test]
fn saved_invalid_source_reopens_as_text_with_its_checkpoint_kept() {
    let project = shop();
    let directory = tempfile::tempdir().unwrap();
    prepare_project(directory.path(), &project).unwrap();
    let mut session = ProjectSession::open(directory.path()).unwrap();
    let id = scenario_document(&session, &project.scenarios[0].id);
    let broken = format!("{}\n  broken =", session.document(&id).unwrap().text());
    session.edit_source(&id, &broken, Origin::SESSION).unwrap();
    session.settle(&id).unwrap();
    assert_eq!(
        session.document(&id).unwrap().analysis(),
        AnalysisState::Invalid
    );
    let saved = session.save(&id).unwrap();
    assert!(!saved.checkpoint, "invalid source never seals a checkpoint");
    drop(session);

    let mut session = ProjectSession::open(directory.path()).unwrap();
    let document = session.document(&id).unwrap();
    assert_eq!(document.text(), broken, "saved exactly, including errors");
    assert_eq!(document.model_state(), ModelState::Absent);
    assert!(!document.accepts_model_edits());

    // Fixing the code restores identities from the older checkpoint.
    let fixed = broken.trim_end_matches("\n  broken =").to_owned();
    session.edit_source(&id, &fixed, Origin::SESSION).unwrap();
    session.settle(&id).unwrap();
    assert_eq!(
        model(&session, &id),
        Document::Scenario(project.scenarios[0].clone())
    );
}

#[test]
fn unqualified_component_calls_are_errors_with_an_explicit_pin() {
    let project = shop();
    let directory = tempfile::tempdir().unwrap();
    prepare_project(directory.path(), &project).unwrap();
    let mut session = ProjectSession::open(directory.path()).unwrap();
    let id = scenario_document(&session, &project.scenarios[0].id);
    let text = session.document(&id).unwrap().text().to_owned();
    let handle = &project.components[0].handle;
    let pinned = format!("{handle}@1(");
    assert!(text.contains(&pinned));
    let unpinned = text.replace(&pinned, &format!("{handle}("));
    session
        .edit_source(&id, &unpinned, Origin::SESSION)
        .unwrap();
    session.settle(&id).unwrap();
    let document = session.document(&id).unwrap();
    assert_eq!(document.analysis(), AnalysisState::Invalid);
    let fix = document
        .diagnostics()
        .iter()
        .find_map(|diagnostic| diagnostic.fix.as_ref())
        .unwrap();
    assert!(
        fix.replacement.ends_with("@2"),
        "pins the current revision explicitly"
    );
}

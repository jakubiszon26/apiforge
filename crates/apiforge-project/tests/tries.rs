//! REQ-17a (#607): a try — a request with no file — is held by a [`TrySet`]
//! beside the project session and edited, checked and undone by the code that
//! edits a scenario. Nothing that saves, reloads, hands over or shares the
//! project meets a try or loses it.
use apiforge_core::{
    block::Block,
    edit::{OutputRef, add_node},
    format::{Binding, Graph, Position, Project},
    graph::ORDER_PORT,
};
use apiforge_project::*;
use apiforge_script::{Diagnostic, FAR_READ, Severity, SyntaxTree};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

#[path = "support/language.rs"]
mod language;
use language::write_language;

const ORIGIN: Origin = Origin(7);

// Fixtures -------------------------------------------------------------------

/// One request, as *New request* makes it.
const REQUEST: &str = "scenario \"Request 1\" {\n  begin = start\n  ping = GET \"/ping\"\n}\n";

/// Places blocks where the fixed placer does not and warns of far reads, as
/// the window's placer does: what a try and a scenario take from it shows.
struct Window;

impl Placer for Window {
    fn place(&self, document: &mut Document, fresh: &[String], context: &Project) {
        FixedPlacer.place(document, fresh, context);
        for node in &mut graph(document).nodes {
            if fresh.contains(&node.id) {
                node.position.x += 1000.0;
            }
        }
    }
    fn advise(&self, tree: &SyntaxTree, document: &Document, context: &Project) -> Vec<Diagnostic> {
        apiforge_script::far_read_warnings(tree, document, context)
    }
}

fn window() -> Arc<dyn Placer> {
    Arc::new(Window)
}

/// The demo shop, whose published components a try can call.
fn shop() -> (tempfile::TempDir, ProjectSession) {
    let directory = tempfile::tempdir().unwrap();
    prepare_project(directory.path(), &apiforge_core::demos::shop_project()).unwrap();
    let session = ProjectSession::open_with(directory.path(), window()).unwrap();
    (directory, session)
}

/// A project of one scenario, with one request.
fn small() -> (tempfile::TempDir, ProjectSession, String) {
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create_with(directory.path(), "Small", window()).unwrap();
    let id = session
        .create_document(
            DocumentKind::Scenario,
            "scenarios/main.afs",
            "scenario \"Main\" {\n  begin = start\n  home = GET \"/\"\n}\n",
        )
        .unwrap();
    (directory, session, id)
}

/// The same project read again as one of language 1.
fn older() -> (tempfile::TempDir, ProjectSession) {
    let (directory, session, _) = small();
    drop(session);
    write_language(directory.path(), 1);
    let session = ProjectSession::open_with(directory.path(), window()).unwrap();
    assert!(session.language_problem().is_some());
    (directory, session)
}

fn graph(document: &mut Document) -> &mut Graph {
    match document {
        Document::Scenario(scenario) => &mut scenario.graph,
        Document::Component(component) => &mut component.graph,
    }
}

fn node_id(graph: &Graph, handle: &str) -> String {
    graph
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("a block `{handle}`"))
        .id
        .clone()
}

/// A fresh set with one try in it.
fn tried(text: &str, context: &TryContext) -> (TrySet, String) {
    let mut tries = TrySet::new(window());
    let id = tries.create("Request 1", text, context).unwrap();
    (tries, id)
}

// Edits as the window makes them ---------------------------------------------

type Change = fn(&mut Document, &Project);

#[derive(Clone, Copy)]
enum Step {
    /// The text as it is typed.
    Type(fn(&str) -> String),
    /// An edit of the model, as the panes and the board make it.
    Edit(Change),
    Undo,
    Redo,
    Accept,
}

fn add_request(document: &mut Document, _: &Project) {
    let graph = graph(document);
    let last = graph.nodes.last().unwrap().id.clone();
    let added = add_node(
        graph,
        Block::Request,
        Position { x: 40.0, y: 40.0 },
        None,
        Some(OutputRef {
            node_id: &last,
            port: ORDER_PORT,
        }),
    )
    .expect("the step is added");
    *graph = added.graph;
}

fn set_address(document: &mut Document, _: &Project) {
    let graph = graph(document);
    let id = node_id(graph, "ping");
    let node = graph.nodes.iter_mut().find(|node| node.id == id).unwrap();
    node.inputs.insert(
        "url".into(),
        Binding::Literal {
            value: "/pong".into(),
        },
    );
}

fn set_method(document: &mut Document, _: &Project) {
    let graph = graph(document);
    let id = node_id(graph, "ping");
    let node = graph.nodes.iter_mut().find(|node| node.id == id).unwrap();
    node.config.method = Some(serde_json::from_value("DELETE".into()).unwrap());
}

/// A change of layout alone.
fn push_aside(document: &mut Document, _: &Project) {
    graph(document).nodes[0].position.x += 40.0;
}

/// A name two steps have: not a valid definition.
fn duplicate_name(document: &mut Document, _: &Project) {
    let nodes = &mut graph(document).nodes;
    let name = nodes[0].handle.clone();
    nodes.last_mut().unwrap().handle = name;
}

/// Another kind of document.
fn as_component(document: &mut Document, context: &Project) {
    *document = Document::Component(context.components[0].clone());
}

/// The id a session gives the scenario is the session's: an edit cannot
/// change it.
fn other_identity(document: &mut Document, _: &Project) {
    if let Document::Scenario(scenario) = document {
        scenario.id = "someone-else".into();
    }
}

fn unchanged(_: &mut Document, _: &Project) {}

// What both hold -------------------------------------------------------------

/// The state of a document or a try after a step, with the result of it.
#[derive(Debug, PartialEq)]
struct State {
    result: String,
    text: String,
    version: u64,
    model: Option<String>,
    model_state: ModelState,
    describes_text: bool,
    analysis: AnalysisState,
    diagnostics: Vec<Diagnostic>,
    omitted: usize,
    undo: bool,
    redo: bool,
}

/// A model with the random identifiers of its entities numbered in the order
/// they are written, so that two readings of one text compare equal.
fn model(document: Option<&Document>) -> Option<String> {
    let mut written = match document? {
        Document::Scenario(scenario) => {
            let mut scenario = scenario.clone();
            scenario.id = "entity".into();
            serde_json::to_string_pretty(&scenario).unwrap()
        }
        Document::Component(component) => serde_json::to_string_pretty(component).unwrap(),
    };
    let mut seen = Vec::new();
    for word in written.split('"') {
        let random = word.rsplit_once('-').is_some_and(|(_, digits)| {
            digits.len() == 8 && digits.bytes().all(|byte| byte.is_ascii_hexdigit())
        });
        if random && !seen.contains(&word.to_owned()) {
            seen.push(word.to_owned());
        }
    }
    for (number, id) in seen.iter().enumerate() {
        written = written.replace(id, &format!("id {number}"));
    }
    Some(written)
}

fn outcome<T: std::fmt::Debug>(result: Result<T>) -> String {
    match result {
        Ok(value) => format!("{value:?}"),
        Err(error) => format!("{error:?}"),
    }
}

/// One step in the session, settled as the window settles it.
fn in_session(session: &mut ProjectSession, id: &str, step: Step) -> State {
    let result = match step {
        Step::Type(change) => {
            let text = change(session.document(id).unwrap().text());
            let result = outcome(session.edit_source(id, &text, ORIGIN));
            session.settle(id).unwrap();
            result
        }
        Step::Edit(change) => {
            let mut next = session.document(id).unwrap().model().unwrap().clone();
            change(&mut next, &session.context().clone());
            outcome(session.apply_model_edit(id, next, ORIGIN))
        }
        Step::Undo => outcome(session.undo(id, ORIGIN)),
        Step::Redo => outcome(session.redo(id, ORIGIN)),
        Step::Accept => outcome(session.accept_comment_loss(id)),
    };
    session_state(session, id, result)
}

fn session_state(session: &ProjectSession, id: &str, result: String) -> State {
    let document = session.document(id).unwrap();
    State {
        result,
        text: document.text().to_owned(),
        version: document.source_version(),
        model: model(document.model()),
        model_state: document.model_state(),
        describes_text: document.model_describes_text(),
        analysis: document.analysis(),
        diagnostics: document.diagnostics().to_vec(),
        omitted: document.omitted_diagnostics(),
        undo: document.can_undo(),
        redo: document.can_redo(),
    }
}

/// The same step in the set, which settles at once.
fn in_set(tries: &mut TrySet, id: &str, step: Step, context: &TryContext, of: &Project) -> State {
    let result = match step {
        Step::Type(change) => {
            let text = change(tries.get(id).unwrap().text());
            outcome(tries.edit_source(id, &text, context))
        }
        Step::Edit(change) => {
            let mut next = tries.get(id).unwrap().model().unwrap().clone();
            change(&mut next, of);
            outcome(tries.apply_model_edit(id, next, context))
        }
        Step::Undo => outcome(tries.undo(id, context)),
        Step::Redo => outcome(tries.redo(id, context)),
        Step::Accept => outcome(tries.accept_comment_loss(id)),
    };
    set_state(tries, id, result)
}

fn set_state(tries: &TrySet, id: &str, result: String) -> State {
    let held = tries.get(id).unwrap();
    State {
        result,
        text: held.text().to_owned(),
        version: held.source_version(),
        model: model(held.model()),
        model_state: held.model_state(),
        describes_text: held.model_describes_text(),
        analysis: held.analysis(),
        diagnostics: held.diagnostics().to_vec(),
        omitted: held.omitted_diagnostics(),
        undo: held.can_undo(),
        redo: held.can_redo(),
    }
}

/// Runs `steps` on a scenario of `session` and on a try of the same text and
/// compares them after each.
fn same(name: &str, session: &mut ProjectSession, text: &str, steps: &[Step]) -> Vec<State> {
    let path = format!("req17a/{name}.afs");
    let id = session
        .create_document(DocumentKind::Scenario, &path, text)
        .unwrap();
    let context = session.try_context();
    let project = session.context().clone();
    let (mut tries, held) = tried(text, &context);
    let mut states = vec![session_state(session, &id, "created".into())];
    assert_eq!(
        set_state(&tries, &held, "created".into()),
        states[0],
        "{name}: created"
    );
    for (number, step) in steps.iter().enumerate() {
        let expected = in_session(session, &id, *step);
        let actual = in_set(&mut tries, &held, *step, &context, &project);
        assert_eq!(actual, expected, "{name}: step {number}");
        states.push(expected);
    }
    states
}

/// Every file under `root`, with its bytes.
fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, at: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, files);
            } else {
                let relative = path.strip_prefix(root).unwrap().to_owned();
                files.insert(relative, fs::read(&path).unwrap_or_default());
            }
        }
    }
    let mut found = BTreeMap::new();
    walk(root, root, &mut found);
    found
}

// Criterion 1 ----------------------------------------------------------------

#[test]
fn req17a_criterion_1_a_set_makes_lists_renames_and_discards_tries() {
    let (_directory, session, _) = small();
    let context = session.try_context();
    let mut tries = TrySet::new(window());
    assert!(tries.list().is_empty());

    let first = tries.create("Request 1", REQUEST, &context).unwrap();
    let second = tries.create("Request 2", "", &context).unwrap();
    assert_ne!(first, second);
    let names = |tries: &TrySet| -> Vec<String> {
        tries
            .list()
            .iter()
            .map(|held| held.name().to_owned())
            .collect()
    };
    assert_eq!(names(&tries), ["Request 1", "Request 2"]);
    assert_eq!(tries.get(&first).unwrap().id(), first);
    assert_eq!(tries.get(&first).unwrap().text(), REQUEST);
    assert!(!tries.get(&first).unwrap().can_undo());

    tries.rename(&first, "Cart").unwrap();
    assert_eq!(names(&tries), ["Cart", "Request 2"]);
    assert_eq!(
        tries.get(&first).unwrap().text(),
        REQUEST,
        "the name of a try is not written in its text"
    );

    tries.discard(&first).unwrap();
    assert_eq!(names(&tries), ["Request 2"]);
    assert!(tries.get(&first).is_none());
    assert!(matches!(tries.discard(&first), Err(Error::UnknownTry)));
    assert!(matches!(tries.rename(&first, "x"), Err(Error::UnknownTry)));
    assert!(matches!(
        tries.edit_source(&first, "", &context),
        Err(Error::UnknownTry)
    ));
    assert!(matches!(
        tries.undo(&first, &context),
        Err(Error::UnknownTry)
    ));
    assert!(matches!(tries.into_source(&first), Err(Error::UnknownTry)));
}

#[test]
fn req17a_criterion_1_a_set_has_no_store_and_no_path_on_disk() {
    let source = include_str!("../src/tries.rs");
    for name in ["ProjectStore", "ProjectPath", "DocumentEntry", "std::fs"] {
        assert!(!source.contains(name), "tries.rs names {name}");
    }
}

// Criterion 2 ----------------------------------------------------------------

/// A text of the language with a warning of its own and two far reads.
const WARNED: &str = "scenario \"S\" {\n  begin = start\n  ping = GET \"/ping\"\n  b = GET \"/b\"\n  c = GET \"/c/{{ping.body.id}}\" {\n    body { again: ping.body.id }\n  }\n  sum = c.status + \"b\"\n}\n";

#[test]
fn req17a_criterion_2_a_try_is_edited_as_a_scenario_is() {
    let typed: fn(&str) -> String = |text| {
        text.replace(
            "GET \"/ping\"",
            "GET \"/ping\" {\n    query { limit: 20 }\n  }",
        )
    };
    let commented: fn(&str) -> String = |text| text.replace("  ping", "  // First.\n  ping");
    let broken: fn(&str) -> String = |text| text.replace("GET", "GET GET");
    let mended: fn(&str) -> String = |text| text.replace("GET GET", "GET");
    let same_text: fn(&str) -> String = |text| text.to_owned();
    let too_large: fn(&str) -> String =
        |_| "x".repeat(apiforge_storage::project::MAX_SOURCE_BYTES + 1);
    use Step::*;
    let table: &[(&str, &str, &[Step])] = &[
        ("no-edit", REQUEST, &[Undo, Redo]),
        (
            "typed",
            REQUEST,
            &[
                Type(typed),
                Type(commented),
                Type(same_text),
                Undo,
                Undo,
                Undo,
                Redo,
                Redo,
                Redo,
            ],
        ),
        (
            "the-model",
            REQUEST,
            &[
                Edit(set_address),
                Edit(set_method),
                Edit(add_request),
                Undo,
                Undo,
                Redo,
                Edit(set_method),
                Edit(push_aside),
                Undo,
                Redo,
            ],
        ),
        (
            "both",
            REQUEST,
            &[
                Edit(set_address),
                Type(typed),
                Edit(add_request),
                Undo,
                Undo,
                Undo,
                Redo,
                Redo,
                Redo,
            ],
        ),
        ("nothing-changed", REQUEST, &[Edit(unchanged), Undo]),
        ("layout", REQUEST, &[Edit(push_aside), Undo, Redo]),
        (
            "an-identity-is-its-own",
            REQUEST,
            &[Edit(other_identity), Undo],
        ),
        (
            "refused",
            REQUEST,
            &[
                Edit(set_address),
                Edit(duplicate_name),
                Edit(as_component),
                Type(too_large),
                Undo,
            ],
        ),
        (
            "invalid-text",
            REQUEST,
            &[
                Edit(set_address),
                Type(broken),
                Edit(set_method),
                Undo,
                Redo,
                Accept,
                Type(mended),
                Edit(set_method),
                Undo,
                Undo,
                Undo,
            ],
        ),
        (
            "warned",
            WARNED,
            &[Edit(set_method), Edit(push_aside), Undo, Undo, Redo],
        ),
        (
            "empty",
            "",
            &[Type(|_| REQUEST.to_owned()), Edit(set_method), Undo, Undo],
        ),
        (
            "no-start",
            "scenario \"S\" {\n  ping = GET \"/ping\"\n}\n",
            &[Type(|_| REQUEST.to_owned()), Edit(set_method)],
        ),
        (
            "a-component-is-no-try",
            "component other \"Other\" {\n  answer = 7\n}\n",
            &[Undo, Type(|_| REQUEST.to_owned())],
        ),
    ];
    let (_directory, mut session) = shop();
    for (name, text, steps) in table {
        same(name, &mut session, text, steps);
    }
}

/// The table compares something: the edits are made, the warnings are there
/// and the placer of the caller is the one that places.
#[test]
fn req17a_criterion_2_the_edits_compared_are_edits() {
    let (_directory, mut session) = shop();
    let states = same(
        "edits",
        &mut session,
        WARNED,
        &[
            Step::Edit(set_method),
            Step::Edit(add_request),
            Step::Edit(duplicate_name),
            Step::Undo,
        ],
    );
    let far = |state: &State| {
        state
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.message == FAR_READ)
            .count()
    };
    assert_eq!(far(&states[0]), 2, "{:?}", states[0].diagnostics);
    assert_eq!(
        states[1].text,
        WARNED.replace("GET \"/ping\"", "DELETE \"/ping\"")
    );
    assert_eq!(far(&states[1]), 2);
    assert_ne!(states[2].text, states[1].text);
    assert_ne!(states[3].result, "()", "a refused edit");
    assert_eq!(states[3].text, states[2].text);
    assert_eq!(states[4].text, states[1].text);

    let context = session.try_context();
    let (tries, id) = tried(REQUEST, &context);
    let mut held = tries.get(&id).unwrap().model().unwrap().clone();
    assert!(
        graph(&mut held)
            .nodes
            .iter()
            .any(|node| node.position.x >= 1000.0),
        "placed by the set's own placer"
    );
}

/// A try calls the project's published components, as a scenario does.
#[test]
fn req17a_criterion_2_a_call_of_a_component_is_read_the_same() {
    let (_directory, mut session) = shop();
    let call = session
        .documents()
        .iter()
        .find(|document| document.entry().kind == DocumentKind::Scenario)
        .unwrap()
        .text()
        .to_owned();
    assert!(call.contains("product@1"), "{call}");
    let states = same(
        "call",
        &mut session,
        &call,
        &[Step::Edit(add_request), Step::Edit(push_aside), Step::Undo],
    );
    assert!(
        states
            .iter()
            .all(|state| state.analysis == AnalysisState::Valid)
    );
}

/// In a project in an older language both give the same diagnostics, whose
/// sentences and fixes are that project's (LANG-13, A5).
#[test]
fn req17a_criterion_2_a_project_in_an_older_language_reads_a_try_as_a_scenario() {
    let (directory, session) = older();
    let text =
        "scenario \"S\" {\n  ping = GET \"/ping\"\n  @after(ping)\n  pong = GET \"/pong\"\n}\n";
    let context = session.try_context();
    let (tries, id) = tried(text, &context);
    let held = tries.get(&id).unwrap();
    drop(session);

    // The session reads the text from disk: an older project is not written.
    fs::write(directory.path().join("scenarios/main.afs"), text).unwrap();
    let session = ProjectSession::open_with(directory.path(), window()).unwrap();
    let document = &session.documents()[0];
    assert_eq!(document.text(), text);
    assert_eq!(held.diagnostics(), document.diagnostics());
    assert!(!held.diagnostics().is_empty());
    assert!(
        held.diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("apiforge fix --folder"))
    );

    // In a project of this build's language the same text is read with its
    // own sentences.
    let (_other, session, _) = small();
    let (tries, id) = tried(text, &session.try_context());
    assert_ne!(
        tries.get(&id).unwrap().diagnostics(),
        document.diagnostics()
    );
}

// Criterion 3 ----------------------------------------------------------------

/// A statement that shares its line with the closing brace: a step after it
/// is written by printing the whole file, which drops the loose note.
const LOOSE: &str =
    "// a loose note\n\nscenario \"Loose\" {\n  begin = start\n  ping = GET \"/ping\" }\n";

#[test]
fn req17a_criterion_3_a_print_that_would_lose_a_comment_is_refused_as_in_the_session() {
    let typed: fn(&str) -> String = |text| text.replace("/ping", "/pong");
    let back: fn(&str) -> String = |text| text.replace("/pong", "/ping");
    let (_directory, mut session) = shop();
    let states = same(
        "loose",
        &mut session,
        LOOSE,
        &[
            Step::Edit(add_request),
            // Written into the text: nothing is asked.
            Step::Edit(set_method),
            Step::Undo,
            // Accepted for one version of the text only.
            Step::Accept,
            Step::Type(typed),
            Step::Edit(add_request),
            Step::Type(back),
            Step::Edit(add_request),
            Step::Accept,
            Step::Edit(add_request),
            Step::Undo,
        ],
    );
    let refused = "CommentsWouldBeLost { comments: 1 }";
    let refusals: Vec<_> = states
        .iter()
        .map(|state| state.result.as_str() == refused)
        .collect();
    assert_eq!(
        refusals,
        [
            false, true, false, false, false, false, true, false, true, false, false, false
        ]
    );
    assert_eq!(states[1].text, LOOSE, "nothing written before the answer");
    assert!(!states[10].text.contains("a loose note"));
    assert!(
        states[11].text.contains("a loose note"),
        "Undo brings it back"
    );
}

#[test]
fn req17a_criterion_3_the_comments_at_stake_are_the_sessions() {
    let (_directory, session, _) = small();
    let (mut tries, id) = tried(LOOSE, &session.try_context());
    assert_eq!(
        tries.get(&id).unwrap().unattached_comments(),
        ["// a loose note"]
    );
    tries
        .edit_source(&id, "scenario {", &session.try_context())
        .unwrap();
    assert!(tries.get(&id).unwrap().unattached_comments().is_empty());
    assert!(matches!(
        tries.accept_comment_loss(&id),
        Err(Error::ModelLocked)
    ));
}

// Criterion 4 ----------------------------------------------------------------

#[test]
fn req17a_criterion_4_a_try_is_analysed_on_every_change_at_once() {
    let (_directory, session, _) = small();
    let context = session.try_context();
    let (mut tries, id) = tried(REQUEST, &context);
    assert_eq!(tries.get(&id).unwrap().analysis(), AnalysisState::Valid);

    tries.edit_source(&id, "scenario {", &context).unwrap();
    let held = tries.get(&id).unwrap();
    assert_eq!(held.analysis(), AnalysisState::Invalid);
    assert_eq!(held.model_state(), ModelState::LastValid);
    assert!(!held.diagnostics().is_empty());

    tries.edit_source(&id, REQUEST, &context).unwrap();
    let held = tries.get(&id).unwrap();
    assert_eq!(held.analysis(), AnalysisState::Valid);
    assert!(held.diagnostics().is_empty());
}

#[test]
fn req17a_criterion_4_refresh_analyses_every_try_again_after_a_publication() {
    let (_directory, mut session, _) = small();
    let calls = |revision: u32| {
        format!("scenario \"S\" {{\n  begin = start\n  got = sign_in@{revision}()\n}}\n")
    };
    let component = session
        .create_document(
            DocumentKind::Component,
            "components/sign-in.afs",
            "component sign_in \"Sign in\" {\n  token = GET \"/token\" {\n    sample 200 {}\n  }\n}\n",
        )
        .unwrap();
    let before = session.try_context();
    let mut tries = TrySet::new(window());
    let first = tries.create("Request 1", &calls(1), &before).unwrap();
    let second = tries.create("Request 2", REQUEST, &before).unwrap();
    assert_eq!(
        tries.get(&first).unwrap().analysis(),
        AnalysisState::Invalid
    );

    session.publish(&component, ORIGIN).unwrap();
    tries.refresh(&session.try_context());
    assert_eq!(
        tries.get(&first).unwrap().analysis(),
        AnalysisState::Valid,
        "{:?}",
        tries.get(&first).unwrap().diagnostics()
    );
    assert_eq!(tries.get(&second).unwrap().analysis(), AnalysisState::Valid);
    assert!(
        !tries.get(&second).unwrap().can_undo(),
        "no step of its own"
    );
    assert_eq!(tries.get(&first).unwrap().source_version(), 1);

    // The context of a project without it.
    tries.refresh(&before);
    assert_eq!(
        tries.get(&first).unwrap().analysis(),
        AnalysisState::Invalid
    );
    assert_eq!(tries.get(&first).unwrap().text(), calls(1));
}

#[test]
fn req17a_criterion_4_refresh_keeps_the_identities_and_the_undo_of_a_try() {
    let (_directory, mut session, _) = small();
    let (mut tries, id) = tried(REQUEST, &session.try_context());
    tries
        .edit_source(
            &id,
            &REQUEST.replace("ping", "pong"),
            &session.try_context(),
        )
        .unwrap();
    let before = tries.clone();
    session.reload_project().unwrap();
    tries.refresh(&session.try_context());
    assert_eq!(tries, before, "the same context reads the same");
    assert!(tries.undo(&id, &session.try_context()).unwrap());
    assert_eq!(tries.get(&id).unwrap().text(), REQUEST);
}

// Criterion 5 ----------------------------------------------------------------

#[test]
fn req17a_criterion_5_a_run_gets_the_context_with_the_try_as_its_one_scenario() {
    let (_directory, session) = shop();
    let context = session.try_context();
    let (tries, id) = tried(REQUEST, &context);
    let project = tries.run_project(&id, &context).unwrap();
    let [scenario] = &project.scenarios[..] else {
        panic!("one scenario, {:?}", project.scenarios.len())
    };
    let Some(Document::Scenario(held)) = tries.get(&id).unwrap().model() else {
        panic!("a scenario")
    };
    assert_eq!(scenario, held);
    let mut without = project.clone();
    without.scenarios.clear();
    assert_eq!(&without, session.context().as_ref());
    assert!(!without.components.is_empty());
    assert!(matches!(
        tries.run_project("other", &context),
        Err(Error::UnknownTry)
    ));
}

#[test]
fn req17a_criterion_5_a_try_that_does_not_check_gives_its_first_diagnostic() {
    let (_directory, session, _) = small();
    let context = session.try_context();
    let (mut tries, id) = tried(REQUEST, &context);
    tries.edit_source(&id, "scenario {", &context).unwrap();
    let first = tries
        .get(&id)
        .unwrap()
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.severity == Severity::Error)
        .unwrap()
        .clone();
    let error = tries.run_project(&id, &context).unwrap_err();
    assert_eq!(error.to_string(), first.message);
    assert!(matches!(error, Error::TryDoesNotCheck { diagnostic } if *diagnostic == first));
}

/// A context that changed since the try was last analysed is the one that
/// decides.
#[test]
fn req17a_criterion_5_a_run_is_checked_in_the_context_it_is_given() {
    let (_directory, mut session, _) = small();
    let component = session
        .create_document(
            DocumentKind::Component,
            "components/sign-in.afs",
            "component sign_in \"Sign in\" {\n  token = GET \"/token\" {\n    sample 200 {}\n  }\n}\n",
        )
        .unwrap();
    let before = session.try_context();
    session.publish(&component, ORIGIN).unwrap();
    let after = session.try_context();
    let (tries, id) = tried(
        "scenario \"S\" {\n  begin = start\n  got = sign_in@1()\n}\n",
        &after,
    );
    assert!(tries.run_project(&id, &after).is_ok());
    assert!(matches!(
        tries.run_project(&id, &before),
        Err(Error::TryDoesNotCheck { .. })
    ));
}

#[test]
fn req17a_criterion_5_a_project_in_an_older_language_refuses_a_run_as_prepare_run_does() {
    let (_directory, mut session) = older();
    let context = session.try_context();
    let document = session.documents()[0].id().to_owned();
    let expected = session.prepare_run(&document).unwrap_err().to_string();
    for text in [REQUEST, "scenario {"] {
        let (tries, id) = tried(text, &context);
        let error = tries.run_project(&id, &context).unwrap_err();
        assert!(matches!(error, Error::OlderLanguage { project: 1, .. }));
        assert_eq!(error.to_string(), expected);
    }
}

// Criterion 6 ----------------------------------------------------------------

#[test]
fn req17a_criterion_6_the_text_of_a_try_registers_as_a_scenario() {
    let (_directory, mut session, _) = small();
    let context = session.try_context();
    let (mut tries, id) = tried(REQUEST, &context);
    tries
        .edit_source(&id, &REQUEST.replace("/ping", "/pong"), &context)
        .unwrap();
    let text = tries.into_source(&id).unwrap();
    assert_eq!(text, REQUEST.replace("/ping", "/pong"));

    // Refused, as a name that is taken is: the try is not lost.
    assert!(
        session
            .create_document(DocumentKind::Scenario, "scenarios/main.afs", &text)
            .is_err()
    );
    assert!(tries.get(&id).unwrap().can_undo());

    let scenario = session
        .create_document(DocumentKind::Scenario, "scenarios/kept.afs", &text)
        .unwrap();
    tries.discard(&id).unwrap();
    let document = session.document(&scenario).unwrap();
    assert_eq!(document.text(), text);
    assert_eq!(document.analysis(), AnalysisState::Valid);
}

// Criterion 7 ----------------------------------------------------------------

/// What only the tries of these tests hold.
const MARK: &str = "req17a-mark";

/// A set with a try that has an undo, one that does not check and one that
/// was accepted to lose a comment.
fn marked(context: &TryContext) -> TrySet {
    let mut tries = TrySet::new(window());
    let text = REQUEST.replace("/ping", &format!("/{MARK}"));
    let first = tries.create(MARK, &text, context).unwrap();
    let mut model = tries.get(&first).unwrap().model().unwrap().clone();
    let project = tries.run_project(&first, context).unwrap();
    set_method(&mut model, &project);
    tries.apply_model_edit(&first, model, context).unwrap();
    assert!(tries.get(&first).unwrap().can_undo());
    let broken = format!("scenario \"{MARK}\" {{");
    tries.create("Request 2", &broken, context).unwrap();
    let loose = LOOSE.replace("a loose note", MARK);
    let third = tries.create("Request 3", &loose, context).unwrap();
    tries.accept_comment_loss(&third).unwrap();
    tries
}

/// `after` is the set as `before` was, and the folder has nothing of it.
fn untouched(directory: &Path, before: &TrySet, after: &TrySet) {
    assert_eq!(after, before);
    for (path, bytes) in files(directory) {
        assert!(
            !String::from_utf8_lossy(&bytes).contains(MARK),
            "{} holds a try",
            path.display()
        );
    }
}

/// The tries are edited and undone in the context of the session as it is
/// now.
fn still_edited(session: &ProjectSession, mut tries: TrySet) {
    let context = session.try_context();
    tries.refresh(&context);
    let id = tries.list()[0].id().to_owned();
    let text = tries.get(&id).unwrap().text().to_owned();
    assert!(tries.undo(&id, &context).unwrap());
    assert!(tries.redo(&id, &context).unwrap());
    assert_eq!(tries.get(&id).unwrap().text(), text);
}

fn type_in(session: &mut ProjectSession, id: &str) {
    let text = session
        .document(id)
        .unwrap()
        .text()
        .replace("GET \"/\"", "GET \"/home\"");
    session.edit_source(id, &text, ORIGIN).unwrap();
}

#[test]
fn req17a_criterion_7_a_save() {
    let (directory, mut session, id) = small();
    let tries = marked(&session.try_context());
    let before = tries.clone();
    type_in(&mut session, &id);
    session.save_recovery().unwrap();
    session.save(&id).unwrap();
    untouched(directory.path(), &before, &tries);
    assert!(!session.has_unsaved_work(), "a try is no unsaved work");
    still_edited(&session, tries);
}

/// *Save all* as the window does it: every document with work, the project
/// text files and the configuration, in one background save.
#[test]
fn req17a_criterion_7_save_all() {
    let (directory, mut session, id) = small();
    let tries = marked(&session.try_context());
    let before = tries.clone();
    type_in(&mut session, &id);
    session
        .edit_configuration(|manifest| manifest.name = "Renamed".into(), ORIGIN)
        .unwrap();
    let completion = session
        .storage_snapshot()
        .run(StorageMode::PreserveEdits, |candidate| {
            let pending: Vec<_> = candidate
                .documents()
                .iter()
                .filter(|document| document.is_dirty() || document.is_layout_dirty())
                .map(|document| document.id().to_owned())
                .collect();
            assert_eq!(pending, [id]);
            for id in pending {
                candidate.save(&id)?;
            }
            candidate.save_configuration()
        });
    session.complete_storage(completion).unwrap().unwrap();
    assert!(!session.has_unsaved_work());
    untouched(directory.path(), &before, &tries);
    still_edited(&session, tries);
}

#[test]
fn req17a_criterion_7_a_background_save_finishing() {
    let (directory, mut session, id) = small();
    let context = session.try_context();
    let mut tries = marked(&context);
    type_in(&mut session, &id);
    let snapshot = session.storage_snapshot();

    // Edited while the save runs: the finished save does not take it back.
    let edited = tries.list()[1].id().to_owned();
    let text = REQUEST.replace("/ping", &format!("/{MARK}"));
    tries.edit_source(&edited, &text, &context).unwrap();
    let before = tries.clone();
    let completion = snapshot.run(StorageMode::PreserveEdits, |candidate| candidate.save(&id));
    session.complete_storage(completion).unwrap().unwrap();
    untouched(directory.path(), &before, &tries);

    let completion = session
        .storage_snapshot()
        .run(StorageMode::Exclusive, |_| ());
    session.complete_storage(completion).unwrap();
    untouched(directory.path(), &before, &tries);
    still_edited(&session, tries);
}

#[test]
fn req17a_criterion_7_a_reload_of_the_project() {
    let (directory, mut session, _) = small();
    let tries = marked(&session.try_context());
    let before = tries.clone();
    session.reload_project().unwrap();
    untouched(directory.path(), &before, &tries);
    session.reload().unwrap();
    untouched(directory.path(), &before, &tries);
    still_edited(&session, tries);
}

#[test]
fn req17a_criterion_7_the_project_handed_over() {
    let (directory, mut session, _) = small();
    let tries = marked(&session.try_context());
    let before = tries.clone();
    assert!(
        session.release_writer().unwrap(),
        "a try holds nothing back"
    );
    assert!(session.is_read_only());
    untouched(directory.path(), &before, &tries);
    still_edited(&session, tries);
}

#[test]
fn req17a_criterion_7_the_project_taken_back() {
    let (directory, mut session, _) = small();
    let tries = marked(&session.try_context());
    let before = tries.clone();
    assert!(session.release_writer().unwrap());
    assert!(session.take_writer(&Holder::window()).unwrap());
    assert!(!session.is_read_only());
    untouched(directory.path(), &before, &tries);
    still_edited(&session, tries);
}

/// The session knows the context of a try and nothing else of one: no field
/// and no event of its own.
#[test]
fn req17a_criterion_7_the_session_gives_the_context_and_holds_no_try() {
    let source = include_str!("../src/session.rs");
    let block = |start: &str| {
        let from = source.find(start).expect(start);
        let to = source[from..].find("\n}\n").unwrap();
        &source[from..from + to]
    };
    for start in ["pub struct ProjectSession {", "pub enum EventKind {"] {
        for name in ["Try", "tries", "try_"] {
            assert!(!block(start).contains(name), "{start} names {name}");
        }
    }
    assert!(!source.contains("TrySet"));
    assert_eq!(source.matches("pub fn try_context(&self)").count(), 1);
}

// Criterion 8 ----------------------------------------------------------------

#[test]
fn req17a_criterion_8_the_command_line_and_the_mcp_server_do_not_name_a_try() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut read = BTreeSet::new();
    for name in ["apiforge-cli", "apiforge-mcp"] {
        for (path, bytes) in files(&crates.join(name).join("src")) {
            let source = String::from_utf8(bytes).unwrap();
            assert!(!source.contains("TrySet"), "{name}: {}", path.display());
            assert!(
                !source.contains("try_context"),
                "{name}: {}",
                path.display()
            );
            read.insert(name);
        }
    }
    assert_eq!(read.len(), 2);
}

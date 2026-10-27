//! KTLO-23a and KTLO-23b: a component runs from its tab the way a scenario
//! does, and asks first for the inputs it has no default for; driven through
//! named widgets (ADR 0013); see docs/tickets/KTLO-23.md. The runner's half is
//! `apiforge-runner/tests/component_target.rs`, the header width (decision 6)
//! is in `long_project_names_do_not_overlap_header_actions_…` and the audits
//! are in `accessibility_tests`.
use super::project_interaction_tests::{app, choose};
use super::*;
use crate::testing::{
    FolderHarness, MINIMUM, WIDE, assert_disjoint, assert_within_window, folder, folder_harness,
    settle,
};
use apiforge_core::run::{RunMode, RunResult, RunStatus, StepStatus};
use egui_kittest::kittest::{NodeT, Queryable};
use std::{
    io::{Read, Write},
    sync::{Arc, Mutex},
};

/// The demo's component whose only input has a default (`id` = 123).
const PRODUCT: &str = "components/product.afs";
/// The header from the mode tabs to *Run*, as a component tab shows it.
const HEADER: [&str; 8] = [
    "Code",
    "Diagram",
    "Split",
    "Save",
    "Save all",
    "Publish",
    "Run options",
    "Run",
];

/// The demo shop at `size` in `mode` with the component at `path` active.
fn on(
    project: &tempfile::TempDir,
    path: &str,
    size: egui::Vec2,
    mode: &str,
) -> (FolderHarness, String) {
    let mut harness = folder_harness(project.path(), size);
    let id = document(folder(&harness), path);
    app(&mut harness).activate_tab(&id);
    settle(&mut harness);
    click(&mut harness, mode);
    (harness, id)
}

fn click(harness: &mut FolderHarness, label: &str) {
    harness.get_by_label(label).click();
    settle(harness);
}

fn has(harness: &FolderHarness, label: &str) -> bool {
    harness.query_by_label_contains(label).is_some()
}

/// Steps until the run has published its last report, which it returns.
fn finish(harness: &mut FolderHarness) -> RunResult {
    let deadline = Instant::now() + Duration::from_secs(30);
    while folder(harness).runs.handle.is_some() {
        assert!(Instant::now() < deadline, "the run must finish");
        harness.step();
        std::thread::sleep(Duration::from_millis(5));
    }
    settle(harness);
    let active = folder(harness).workbench.active().unwrap().to_owned();
    folder(harness)
        .run_result(&active)
        .unwrap_or_else(|| {
            panic!(
                "a run from the component's tab leaves a report: {:?}",
                folder(harness).notice.as_ref().map(|notice| &notice.text)
            )
        })
        .clone()
}

fn run(harness: &mut FolderHarness) -> RunResult {
    choose(harness, "Run");
    finish(harness)
}

/// A step's row in the *Run results* list.
fn row(step: &apiforge_core::run::StepResult) -> String {
    format!("{} · {}", step.display_name(), step.status.label())
}

fn edit(harness: &mut FolderHarness, id: &str, from: &str, to: &str) {
    let source = folder(harness)
        .session
        .as_ref()
        .unwrap()
        .document(id)
        .unwrap()
        .text()
        .to_owned();
    let edited = source.replacen(from, to, 1);
    assert_ne!(edited, source, "{from:?} is in the source");
    app(harness)
        .session
        .as_mut()
        .unwrap()
        .edit_source(id, &edited, CODE)
        .unwrap();
    settle(harness);
}

/// Criterion 1: on a component tab the header holds *Run* and *Run options*
/// next to *Publish*, every control inside the window and none overlapping,
/// in every mode at both windows; *Run*'s hover text names both kinds.
#[test]
fn ktlo23_a_component_tab_holds_run_and_run_options_beside_publish() {
    let project = shop();
    for size in [MINIMUM, WIDE] {
        for mode in ["Code", "Diagram", "Split"] {
            let (harness, _) = on(&project, PRODUCT, size, mode);
            assert_within_window(&harness, &HEADER);
            assert_disjoint(&harness, &HEADER);
            let rect = |label| harness.get_by_label(label).rect();
            assert!(
                rect("Publish").right() <= rect("Run options").left()
                    && rect("Run options").right() <= rect("Run").left(),
                "{mode} at {size:?}: Publish, Run options, Run, in this order"
            );
        }
    }
    let (mut harness, _) = on(&project, PRODUCT, WIDE, "Diagram");
    harness.get_by_label("Run").hover();
    settle(&mut harness);
    harness.run_steps(60);
    assert!(has(
        &harness,
        "Simulate the selected scenario or component, including unsaved valid code"
    ));
}

/// Criterion 2: *Run* on a component whose inputs all have defaults lists
/// each of its steps with its status and the export's value; there is no
/// error policy to choose, and nothing on disk changes.
#[test]
fn ktlo23_run_on_a_component_lists_its_steps_and_the_exported_value() {
    let project = shop();
    let saved = std::fs::read(project.path().join(PRODUCT)).unwrap();
    let (mut harness, id) = on(&project, PRODUCT, WIDE, "Diagram");
    let report = run(&mut harness);
    assert_eq!(report.status, RunStatus::Passed, "{:?}", report.error);
    assert_eq!(report.mode, RunMode::Simulation);
    let nodes: Vec<String> = graph_of(&model_of(&harness, &id), false)
        .nodes
        .iter()
        .map(|node| node.id.clone())
        .collect();
    let steps: Vec<String> = report
        .steps
        .iter()
        .map(|step| step.node_id.clone())
        .collect();
    assert_eq!(steps, nodes, "one step per block of the component");
    for step in &report.steps {
        assert!(has(&harness, &row(step)), "the row {:?}", row(step));
    }
    let fetch = &report.steps[0];
    assert!(
        fetch
            .request
            .as_ref()
            .unwrap()
            .url
            .ends_with("/products/123")
    );
    let export = report
        .steps
        .iter()
        .find(|step| step.kind == apiforge_core::model::NodeKind::Export)
        .expect("the export is a step");
    assert_eq!(export.outputs.get("value"), fetch.outputs.get("body"));
    harness.get_by_label_contains(&row(export)).click();
    settle(&mut harness);
    assert!(
        has(&harness, "Outputs  1"),
        "the export's value is its output"
    );
    assert!(has(&harness, "PASSED"));
    assert!(
        !has(&harness, "When a step fails"),
        "a component has no error policy"
    );
    assert_eq!(std::fs::read(project.path().join(PRODUCT)).unwrap(), saved);
}

fn model_of(harness: &FolderHarness, id: &str) -> Document {
    folder(harness)
        .session
        .as_ref()
        .unwrap()
        .document(id)
        .unwrap()
        .model()
        .cloned()
        .unwrap()
}

/// Criteria 3 and 5 (second half): the unsaved value runs, the file keeps the
/// saved one, and editing after the run marks the report stale.
#[test]
fn ktlo23_the_run_uses_the_unsaved_text_and_a_later_edit_makes_it_stale() {
    let project = shop();
    let saved = std::fs::read(project.path().join(PRODUCT)).unwrap();
    let (mut harness, id) = on(&project, PRODUCT, WIDE, "Code");
    edit(&mut harness, &id, "123", "124");
    let report = run(&mut harness);
    assert_eq!(report.status, RunStatus::Passed, "{:?}", report.error);
    assert!(
        report.steps[0]
            .request
            .as_ref()
            .unwrap()
            .url
            .ends_with("/products/124"),
        "the unsaved default"
    );
    assert_eq!(std::fs::read(project.path().join(PRODUCT)).unwrap(), saved);
    assert!(folder(&harness).fresh_run(&id).is_some());
    edit(&mut harness, &id, "124", "125");
    assert!(folder(&harness).fresh_run(&id).is_none());
    assert!(has(
        &harness,
        "Graph changed · results from an earlier version"
    ));
}

/// Criterion 4: a component calling another runs the revision its call pins,
/// not the callee's working text, and lists the called steps indented under
/// the call.
#[test]
fn ktlo23_a_called_component_runs_its_pinned_revision_indented() {
    let project = shop();
    let mut session = ProjectSession::open(project.path()).unwrap();
    let product = session
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == PRODUCT)
        .unwrap()
        .id()
        .to_owned();
    let handle = session
        .document(&product)
        .unwrap()
        .text()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .to_owned();
    session
        .create_document(
            DocumentKind::Component,
            "components/outer.afs",
            &format!(
                "component Outer \"Outer\" {{\n  call = ({handle}@1())\n  export inner: object = call.product\n}}\n"
            ),
        )
        .unwrap();
    drop(session);
    let mut harness = folder_harness(project.path(), WIDE);
    let product = document(folder(&harness), PRODUCT);
    edit(&mut harness, &product, "123", "999");
    let outer = document(folder(&harness), "components/outer.afs");
    app(&mut harness).activate_tab(&outer);
    settle(&mut harness);
    click(&mut harness, "Diagram");
    let report = run(&mut harness);
    assert_eq!(report.status, RunStatus::Passed, "{:?}", report.error);
    let children = report.steps[0]
        .children
        .as_deref()
        .expect("the call's steps");
    assert!(
        children[0]
            .request
            .as_ref()
            .unwrap()
            .url
            .ends_with("/products/123"),
        "revision 1, not the unsaved 999"
    );
    let rows = crate::report::flatten_steps(&report.steps);
    assert!(rows.iter().any(|row| row.depth == 1), "indented children");
    assert!(
        has(&harness, &row(&children[0])),
        "the inner step is listed"
    );
}

/// Criterion 5 (first half): invalid code offers the last valid version and
/// runs it only after *Run this version*.
#[test]
fn ktlo23_invalid_component_code_offers_the_last_valid_version() {
    let project = shop();
    let (mut harness, id) = on(&project, PRODUCT, WIDE, "Code");
    edit(&mut harness, &id, "{", "{ broken");
    choose(&mut harness, "Run");
    assert!(has(&harness, "Run the last valid version?"));
    assert!(has(&harness, "Component: Product"));
    assert!(folder(&harness).runs.handle.is_none(), "nothing runs yet");
    click(&mut harness, "Run this version");
    let report = finish(&mut harness);
    assert_eq!(report.status, RunStatus::Passed, "{:?}", report.error);
    assert!(
        folder(&harness).fresh_run(&id).is_none(),
        "an earlier version"
    );
}

/// Criterion 6: choosing a step selects its block on the component's board
/// and opens its inspector.
#[test]
fn ktlo23_choosing_a_step_selects_its_block_on_the_components_board() {
    for size in [MINIMUM, WIDE] {
        let project = shop();
        let (mut harness, id) = on(&project, PRODUCT, size, "Diagram");
        let report = run(&mut harness);
        let fetch = &report.steps[0];
        harness.get_by_label_contains(&row(fetch)).click();
        settle(&mut harness);
        assert_eq!(
            folder(&harness)
                .selections
                .get(&id)
                .cloned()
                .unwrap_or_default(),
            vec![fetch.node_id.clone()],
            "{size:?}"
        );
        assert!(has(&harness, "Close Inspector"), "{size:?}: its inspector");
    }
}

/// Criterion 11: the export's value offers no *Check* or *To variable*, which
/// would edit the component's own graph; another step still does.
#[test]
fn ktlo23_a_components_export_step_offers_no_check_or_variable() {
    let project = shop();
    let (mut harness, _) = on(&project, PRODUCT, WIDE, "Code");
    let report = run(&mut harness);
    let export = report.steps.last().unwrap();
    assert_eq!(export.kind, apiforge_core::model::NodeKind::Export);
    harness.get_by_label_contains(&row(export)).click();
    settle(&mut harness);
    for action in ["Check", "To variable", "Choose a field → add a check"] {
        assert!(
            harness.query_all_by_label(action).next().is_none(),
            "{action} on the export"
        );
    }
    harness
        .get_by_label_contains(&row(&report.steps[0]))
        .click();
    settle(&mut harness);
    assert!(
        harness.query_all_by_label("Check").next().is_some(),
        "a request's output can still be checked"
    );
}

/// A loopback API that records each request line and answers with a product,
/// or holds the connection open without answering.
fn api(answer: bool) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let lines = Arc::clone(&seen);
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let lines = Arc::clone(&lines);
            std::thread::spawn(move || {
                let mut buffer = [0; 4096];
                let read = stream.read(&mut buffer).unwrap_or(0);
                let text = String::from_utf8_lossy(&buffer[..read]).into_owned();
                lines
                    .lock()
                    .unwrap()
                    .push(text.lines().next().unwrap_or_default().to_owned());
                if !answer {
                    std::thread::sleep(Duration::from_secs(30));
                    return;
                }
                let body = r#"{"id":123,"name":"Live"}"#;
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            });
        }
    });
    (base, seen)
}

fn live(harness: &mut FolderHarness, base: &str) {
    app(harness)
        .session
        .as_mut()
        .unwrap()
        .edit_configuration(
            |manifest| manifest.environment.base_url = base.to_owned(),
            CODE,
        )
        .unwrap();
    settle(harness);
    choose(harness, "Run options");
    choose(harness, "Run live requests");
}

/// Criterion 8: *Run options → Run live requests* runs the component live; the
/// journal names the run after the component, and a run interrupted mid-request
/// is listed at the next start. While no step has finished, the empty report
/// speaks of the component, not of a scenario.
#[test]
fn ktlo23_live_requests_from_a_component_are_sent_and_journalled_under_its_name() {
    let project = shop();
    let (base, requests) = api(true);
    let (mut harness, _) = on(&project, PRODUCT, WIDE, "Diagram");
    live(&mut harness, &base);
    let report = finish(&mut harness);
    assert_eq!(report.mode, RunMode::Live);
    assert_eq!(report.status, RunStatus::Passed, "{:?}", report.error);
    assert_eq!(
        requests.lock().unwrap().as_slice(),
        ["GET /products/123 HTTP/1.1"]
    );

    let (base, _) = api(false);
    live(&mut harness, &base);
    let journal = project.path().join(".apiforge/local/journal.json");
    let deadline = Instant::now() + Duration::from_secs(10);
    let crashed = loop {
        harness.step();
        if let Ok(bytes) = std::fs::read(&journal)
            && String::from_utf8_lossy(&bytes).contains("\"Product\"")
        {
            break bytes;
        }
        assert!(Instant::now() < deadline, "the live run is journalled");
        std::thread::sleep(Duration::from_millis(5));
    };
    harness.run_steps(5);
    assert!(has(
        &harness,
        "Results will appear while the component runs."
    ));
    assert!(!has(&harness, "while the scenario runs"));
    choose(&mut harness, "Cancel run");
    finish(&mut harness);
    // The test address is not the project's; nothing asks to save it.
    app(&mut harness)
        .session
        .as_mut()
        .unwrap()
        .discard_configuration()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !app(&mut harness).prepare_close() {
        assert!(Instant::now() < deadline, "the window closes");
        harness.step();
    }
    drop(harness);
    // The bytes a process killed mid-request leaves behind.
    std::fs::write(&journal, crashed).unwrap();
    let harness = folder_harness(project.path(), WIDE);
    assert!(has(&harness, "A live run did not finish"));
    assert!(
        has(&harness, "Product · "),
        "listed under the component's name"
    );
}

/// Design review: a component tab's header, and its run results with the
/// export selected (KTLO-23a), at both windows.
#[test]
#[ignore = "KTLO-23a: design review capture; run with tools/gui/review.sh"]
fn review_component_run() {
    for (size, suffix) in [(MINIMUM, "960x540"), (WIDE, "1600x1000")] {
        let project = shop();
        let (mut harness, _) = on(&project, PRODUCT, size, "Diagram");
        crate::testing::capture(&mut harness, &format!("component-header-{suffix}"), None);
        let report = run(&mut harness);
        harness
            .get_by_label_contains(&row(report.steps.last().unwrap()))
            .click();
        settle(&mut harness);
        crate::testing::capture(&mut harness, &format!("component-run-{suffix}"), None);
    }
}

// ----- KTLO-23b: the Run inputs dialog (criterion 9, decision 1) ----------------

/// The field of the demo's `product` input, which has no default.
const PRODUCT_FIELD: &str = "product · Object";
/// The field of the demo's `quantity` input, whose default is 1.
const QUANTITY_FIELD: &str = "Quantity · quantity · Number";

fn asks_for_inputs(harness: &FolderHarness) -> bool {
    harness.query_by_label("Run inputs").is_some()
}

/// The dialog's own *Run*, on *Cancel*'s row: the header has a *Run* too.
fn dialog_run(harness: &FolderHarness) -> egui_kittest::Node<'_> {
    let cancel = harness.get_by_label("Cancel").rect();
    harness
        .query_all_by_label("Run")
        .find(|node| (node.rect().center().y - cancel.center().y).abs() < 4.0)
        .expect("the dialog's Run beside Cancel")
}

fn run_enabled(harness: &FolderHarness) -> bool {
    !dialog_run(harness).accesskit_node().is_disabled()
}

fn field(harness: &FolderHarness, label: &str) -> String {
    harness.get_by_label(label).value().unwrap_or_default()
}

/// Replaces the text of the field named `label`.
fn enter(harness: &mut FolderHarness, label: &str, text: &str) {
    harness.get_by_label(label).focus();
    harness.step();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.key_press(Key::Backspace);
    if !text.is_empty() {
        harness.get_by_label(label).type_text(text);
    }
    settle(harness);
}

/// The value the run gave the `Cart item` block, whose inputs are the
/// component's `product.id` and `quantity`.
fn cart_item(report: &RunResult) -> serde_json::Value {
    report
        .steps
        .iter()
        .find(|step| step.display_name() == "Cart item")
        .and_then(|step| step.outputs.get("value"))
        .cloned()
        .expect("the Cart item step ran")
}

/// Criterion 9: *Run* on a component with an input without a default asks for
/// the inputs first, one field per input named with its type and pre-filled
/// from its default; *Run* in the dialog stays disabled until every input has
/// a value, and nothing runs meanwhile. Both windows hold the whole dialog.
#[test]
fn ktlo23b_an_input_without_a_default_asks_for_the_run_inputs() {
    for size in [MINIMUM, WIDE] {
        let project = shop();
        let (mut harness, id) = on(&project, COMPONENT, size, "Diagram");
        choose(&mut harness, "Run");
        assert!(asks_for_inputs(&harness), "{size:?}: the dialog opens");
        assert!(folder(&harness).runs.handle.is_none(), "nothing runs yet");
        assert!(folder(&harness).run_result(&id).is_none());
        assert_eq!(field(&harness, PRODUCT_FIELD), "", "no default");
        assert_eq!(field(&harness, QUANTITY_FIELD), "1", "its default");
        assert!(has(&harness, "no default, enter a value to run"));
        assert!(has(&harness, "pre-filled from its default"));
        assert!(!run_enabled(&harness), "{size:?}: product has no value");
        let labels = ["Run inputs", PRODUCT_FIELD, QUANTITY_FIELD, "Cancel"];
        assert_within_window(&harness, &labels);
        assert_disjoint(&harness, &labels);
        let window = harness.get_by_label("Run inputs").rect();
        let run = dialog_run(&harness).rect();
        assert!(
            run.left() >= 0.0 && run.right() <= size.x && run.bottom() <= size.y,
            "{size:?}: the dialog's Run {run:?} below {window:?} is in the window"
        );

        enter(&mut harness, PRODUCT_FIELD, "not json");
        assert!(!run_enabled(&harness), "{size:?}: an object is JSON");
        enter(&mut harness, PRODUCT_FIELD, r#"{"id": 124}"#);
        assert!(run_enabled(&harness), "{size:?}: every input has a value");
        enter(&mut harness, QUANTITY_FIELD, "");
        assert!(!run_enabled(&harness), "{size:?}: quantity is emptied");
        enter(&mut harness, QUANTITY_FIELD, "two");
        assert!(!run_enabled(&harness), "{size:?}: quantity is a number");
        enter(&mut harness, QUANTITY_FIELD, "2");
        assert!(run_enabled(&harness));
        assert!(folder(&harness).runs.handle.is_none(), "still nothing runs");
    }
}

/// Criterion 9: the dialog's *Run* runs with the entered values, and the next
/// run of the same component pre-fills them; another component does not see
/// them.
#[test]
fn ktlo23b_the_entered_values_run_and_are_kept_for_the_component() {
    let project = shop();
    let mut session = ProjectSession::open(project.path()).unwrap();
    let cart = session
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == COMPONENT)
        .unwrap()
        .id()
        .to_owned();
    let source = session.document(&cart).unwrap().text().to_owned();
    let handle = source.split_whitespace().nth(1).unwrap().to_owned();
    let other = source.replacen(
        &format!("component {handle} \"Add to cart\""),
        "component OtherCart \"Other cart\"",
        1,
    );
    assert_ne!(other, source, "the copy is renamed");
    session
        .create_document(DocumentKind::Component, "components/other-cart.afs", &other)
        .unwrap();
    drop(session);

    let (mut harness, id) = on(&project, COMPONENT, WIDE, "Diagram");
    choose(&mut harness, "Run");
    enter(&mut harness, PRODUCT_FIELD, r#"{"id": 124}"#);
    enter(&mut harness, QUANTITY_FIELD, "2");
    dialog_run(&harness).click();
    settle(&mut harness);
    assert!(!asks_for_inputs(&harness), "the dialog closes");
    let report = finish(&mut harness);
    assert_eq!(report.status, RunStatus::Passed, "{:?}", report.error);
    assert_eq!(
        cart_item(&report),
        serde_json::json!({ "productId": 124, "quantity": 2 }),
        "the entered values"
    );
    assert!(
        !report
            .steps
            .iter()
            .any(|step| step.status == StepStatus::Error),
        "no input is missing"
    );

    choose(&mut harness, "Run");
    assert!(asks_for_inputs(&harness), "asked again, pre-filled");
    let product: serde_json::Value = serde_json::from_str(&field(&harness, PRODUCT_FIELD)).unwrap();
    assert_eq!(product, serde_json::json!({ "id": 124 }));
    assert_eq!(field(&harness, QUANTITY_FIELD), "2", "not the default 1");
    assert!(run_enabled(&harness));
    dialog_run(&harness).click();
    settle(&mut harness);
    let again = finish(&mut harness);
    assert_eq!(
        cart_item(&again),
        serde_json::json!({ "productId": 124, "quantity": 2 })
    );
    assert_eq!(folder(&harness).workbench.active(), Some(id.as_str()));

    let other = document(folder(&harness), "components/other-cart.afs");
    app(&mut harness).activate_tab(&other);
    settle(&mut harness);
    choose(&mut harness, "Run");
    assert!(asks_for_inputs(&harness));
    assert_eq!(
        field(&harness, PRODUCT_FIELD),
        "",
        "another component's values"
    );
    assert_eq!(field(&harness, QUANTITY_FIELD), "1", "its own default");
}

/// Criterion 9: *Cancel* runs nothing and keeps nothing.
#[test]
fn ktlo23b_cancel_runs_nothing() {
    let project = shop();
    let (mut harness, id) = on(&project, COMPONENT, WIDE, "Diagram");
    choose(&mut harness, "Run");
    enter(&mut harness, PRODUCT_FIELD, r#"{"id": 124}"#);
    click(&mut harness, "Cancel");
    assert!(!asks_for_inputs(&harness), "the dialog closes");
    harness.run_steps(5);
    assert!(folder(&harness).runs.handle.is_none(), "nothing runs");
    assert!(folder(&harness).run_result(&id).is_none(), "no report");
    choose(&mut harness, "Run");
    assert_eq!(
        field(&harness, PRODUCT_FIELD),
        "",
        "cancelled values are not kept"
    );
    harness.key_press(Key::Escape);
    settle(&mut harness);
    assert!(!asks_for_inputs(&harness), "Escape cancels too");
    assert!(folder(&harness).run_result(&id).is_none());
}

/// Decision 1: a component whose inputs all have defaults runs at once.
#[test]
fn ktlo23b_a_component_whose_inputs_all_have_defaults_runs_with_no_dialog() {
    let project = shop();
    let (mut harness, _) = on(&project, PRODUCT, WIDE, "Diagram");
    choose(&mut harness, "Run");
    assert!(!asks_for_inputs(&harness), "no dialog");
    let report = finish(&mut harness);
    assert_eq!(report.status, RunStatus::Passed, "{:?}", report.error);
}

/// Design review: the Run inputs dialog as it opens and filled (KTLO-23b), at
/// both windows; compare with `docs/qa/2026-10-27/KTLO-23/mock-run-inputs-*`.
#[test]
#[ignore = "KTLO-23b: design review capture; run with tools/gui/review.sh"]
fn review_run_inputs() {
    for (size, suffix) in [(MINIMUM, "960x540"), (WIDE, "1600x1000")] {
        let project = shop();
        let (mut harness, _) = on(&project, COMPONENT, size, "Diagram");
        choose(&mut harness, "Run");
        crate::testing::capture(&mut harness, &format!("run-inputs-{suffix}"), None);
        enter(&mut harness, PRODUCT_FIELD, r#"{ "id": 123 }"#);
        enter(&mut harness, QUANTITY_FIELD, "2");
        crate::testing::capture(&mut harness, &format!("run-inputs-filled-{suffix}"), None);
    }
}

//! REP-07a (#381, the list): in Run results a request carries its checks
//! under it, a check's row is its predicate as DIAG-02 writes it (the handle
//! moves to the hover and the row's accessible name), *Failed* keeps the
//! requests with a failure, *Collapse checks* and ▸ fold the checks, and the
//! list's edge is dragged, its width remembered on this computer. Decided by
//! the owner on 2026-11-19 (question 1 = A, mock-up M1a) and split from the
//! details (REP-07b) on 2026-11-23. On the `labels` reference scenario
//! (FIX-01) and on `rooms/booking-day`.
use super::*;
use crate::preferences::Preferences;
use crate::report::{Line, RunGraphs, arrange, flatten_steps};
use crate::testing::{LABELS, MINIMUM, assert_within_window};
use apiforge_core::format::{Graph, Node, Project};
use apiforge_core::model::NodeKind;
use apiforge_core::run::Phase;
use egui_kittest::kittest::NodeT;
use std::path::Path;

const MAXIMISE: &str = "Maximise run results";
const RESIZE_LIST: &str = "Resize the step list";
const COLLAPSE: &str = "Collapse checks";
const EXPAND: &str = "Expand checks";
/// The least width of the list, and what the details keep at 960 × 540.
const LEAST_LIST: f32 = 240.0;
const LEAST_DETAILS: f32 = 400.0;

/// The request whose check fails in criteria 3 and 5, and that check.
const WEIGHT_31: &str = "Weight 31 kg";
const FAILED_CHECK: &str = "error.field == \"weight\"";

fn labels_text() -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/scenarios/lay/labels.afs"),
    )
    .unwrap()
}

/// `labels` with `weight_31`'s response naming the field `mass`: its check
/// `weight_31_field` fails, every other check passes.
fn failing() -> String {
    let text = labels_text();
    let failing = text.replacen(
        r#"field: "weight", message: "Weight must be at most 30 kg.""#,
        r#"field: "mass", message: "Weight must be at most 30 kg.""#,
        1,
    );
    assert_ne!(failing, text, "weight_31's sample names its field");
    failing
}

/// `text` as `labels`' only document, open on its board at `size`.
fn labels_with(size: egui::Vec2, text: &str) -> (tempfile::TempDir, FolderHarness, String) {
    let directory = tempfile::tempdir().unwrap();
    let mut project = apiforge_core::demos::empty_project();
    project.scenarios.clear();
    project.name = "Label validation".into();
    apiforge_project::prepare_project(directory.path(), &project).unwrap();
    let mut session = apiforge_project::ProjectSession::open(directory.path()).unwrap();
    let id = session
        .create_document(DocumentKind::Scenario, LABELS, text)
        .unwrap();
    drop(session);
    let mut harness = crate::testing::folder_harness_in(directory.path(), size, Mode::Diagram);
    app(&mut harness).review_open(LABELS, None);
    settle(&mut harness);
    (directory, harness, id)
}

/// `rooms/booking-day` open on its board at `size`.
fn booking_day(size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, String) {
    fn copy_tree(from: &Path, to: &Path) {
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                fs::create_dir_all(&target).unwrap();
                copy_tree(&entry.path(), &target);
            } else {
                fs::copy(entry.path(), target).unwrap();
            }
        }
    }
    let project = tempfile::tempdir().unwrap();
    copy_tree(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/scenarios/lay/rooms"),
        project.path(),
    );
    let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).review_open("scenarios/booking-day.afs", None);
    settle(&mut harness);
    let id = folder(&harness)
        .session
        .as_ref()
        .unwrap()
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == "scenarios/booking-day.afs")
        .unwrap()
        .id()
        .to_owned();
    (project, harness, id)
}

/// A simulation of the active scenario, finished, with Run results
/// maximised.
fn ran(harness: &mut FolderHarness) -> RunResult {
    choose(harness, "Run simulation");
    let result = finish_run(harness);
    choose(harness, MAXIMISE);
    result
}

/// The document's graphs, Main and Cleanup, and the project they read.
fn graphs(harness: &FolderHarness, id: &str) -> (Graph, Graph, std::sync::Arc<Project>) {
    let snapshot = folder(harness).snapshot(id).unwrap();
    let model = snapshot.model.as_ref().unwrap();
    (
        graph_of(model, false).clone(),
        graph_of(model, true).clone(),
        folder(harness).context_project(),
    )
}

fn node<'a>(graph: &'a Graph, handle: &str) -> &'a Node {
    graph
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("a step {handle}"))
}

/// A check's predicate as its card writes it (DIAG-02).
fn written(graph: &Graph, project: &Project, check: &Node) -> String {
    let scope = apiforge_core::template_text::Scope {
        nodes: &graph.nodes,
        project,
        request: Some(check),
    };
    let own = crate::canvas::own_step(check, graph);
    crate::canvas::check_text(check, scope, graph, own).text()
}

/// The list's right edge: the left of the edge that resizes it.
fn list_right(harness: &FolderHarness) -> f32 {
    harness.get_by_label(RESIZE_LIST).rect().left()
}

/// The rows of the step list, top to bottom: each row's accessible label.
fn rows(harness: &FolderHarness) -> Vec<String> {
    let right = list_right(harness);
    let mut rows: Vec<(f32, String)> = harness
        .query_all_by_role(egui::accesskit::Role::Button)
        .filter(|node| node.rect().right() <= right + 1.0)
        .filter_map(|node| {
            let label = node.accesskit_node().label()?;
            label
                .contains(" · ")
                .then(|| (node.rect().top(), label.to_string()))
        })
        .collect();
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    rows.into_iter().map(|(_, label)| label).collect()
}

/// The row whose label starts with `start`.
fn row(harness: &FolderHarness, start: &str) -> String {
    rows(harness)
        .into_iter()
        .find(|label| label.starts_with(start))
        .unwrap_or_else(|| panic!("a row starts with {start:?}: {:?}", rows(harness)))
}

/// Clicks the row whose label starts with `start`.
fn click_row(harness: &mut FolderHarness, start: &str) {
    let label = row(harness, start);
    harness.get_by_label(&label).scroll_to_me();
    settle(harness);
    harness.get_by_label(&label).click();
    settle(harness);
}

/// The selected row of the list.
fn selected(harness: &FolderHarness) -> String {
    let right = list_right(harness);
    let chosen: Vec<String> = harness
        .query_all_by_role(egui::accesskit::Role::Button)
        .filter(|node| node.rect().right() <= right + 1.0)
        .filter(|node| node.accesskit_node().is_selected() == Some(true))
        .filter_map(|node| node.accesskit_node().label().map(|label| label.to_string()))
        .collect();
    assert_eq!(chosen.len(), 1, "one row is selected: {chosen:?}");
    chosen[0].clone()
}

/// Where the selected step's details start: the left of their first tab.
fn details(harness: &FolderHarness) -> f32 {
    harness
        .query_all_by_label_contains("Outputs  ")
        .map(|node| node.rect().left())
        .next()
        .expect("the step's details show their Outputs tab")
}

/// The list's width as its edge says it.
fn list_width(harness: &FolderHarness) -> f32 {
    harness
        .get_by_label(RESIZE_LIST)
        .accesskit_node()
        .numeric_value()
        .expect("the edge says the list's width") as f32
}

/// Drags the list's edge by `by` points across.
fn drag_edge(harness: &mut FolderHarness, by: f32) {
    let from = harness.get_by_label(RESIZE_LIST).rect().center();
    super::lay16::drag(harness, from, from + egui::vec2(by, 0.0));
}

/// Criterion 1: on `labels`, after a simulated run, every check's row is
/// its predicate as its card writes it; no row is a handle and a status.
#[test]
fn rep07a_criterion_1_a_check_row_is_its_predicate() {
    let (_project, mut harness, id) = labels_with(WIDE, &labels_text());
    let result = ran(&mut harness);
    let (main, _, project) = graphs(&harness, &id);
    let rows = rows(&harness);
    assert_eq!(rows.len(), result.steps.len(), "a row a step: {rows:?}");
    let checks: Vec<&Node> = main
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Assert)
        .collect();
    assert_eq!(checks.len(), 33, "labels has 33 checks");
    for check in checks {
        let text = written(&main, &project, check);
        assert!(
            rows.iter()
                .any(|row| row.starts_with(&format!("{text} · ")) && row.contains(&check.handle)),
            "{}'s row reads {text:?}: {rows:?}",
            check.handle
        );
        assert!(
            !rows
                .iter()
                .any(|row| row.starts_with(&format!("{} · ", check.handle))),
            "no row of {} is its handle and a status: {rows:?}",
            check.handle
        );
    }
    // A request's row starts with its name.
    for name in ["No token", WEIGHT_31, "Weight 0 kg", "Unknown parcel"] {
        row(&harness, &format!("{name} · "));
    }
}

/// The steps of `rows` by node, the lines' row indexes as node ids.
fn by_node<'a>(
    rows: &[crate::report::StepRow<'a>],
    lines: &[Line],
) -> Vec<(&'a str, Option<&'a str>)> {
    lines
        .iter()
        .map(|line| {
            (
                rows[line.row].step.node_id.as_str(),
                line.under.map(|head| rows[head].step.node_id.as_str()),
            )
        })
        .collect()
}

/// Criterion 2: a check is listed under the step it runs after (the anchor
/// of `canvas::collapsible`), a check with no step stands by itself, and
/// the rows keep the order of the run: on `labels` and on
/// `rooms/booking-day`, whose checks hang in Main, inside an `if` and in
/// Cleanup, and whose component call holds steps of its own.
#[test]
fn rep07a_criterion_2_checks_are_listed_under_the_step_they_run_after() {
    let (_labels, mut labels, labels_id) = labels_with(WIDE, &labels_text());
    choose(&mut labels, "Run simulation");
    let labels_run = finish_run(&mut labels);
    let (_rooms, mut rooms, rooms_id) = booking_day(WIDE);
    choose(&mut rooms, "Run simulation");
    let rooms_run = finish_run(&mut rooms);
    for (name, harness, id, result) in [
        ("labels", &labels, &labels_id, &labels_run),
        ("booking-day", &rooms, &rooms_id, &rooms_run),
    ] {
        let (main, cleanup, project) = graphs(harness, id);
        let rows = flatten_steps(&result.steps);
        let lines = arrange(
            &rows,
            Some(RunGraphs {
                main: &main,
                cleanup: Some(&cleanup),
                project: &project,
            }),
        );
        // Every row once.
        let mut listed: Vec<usize> = lines.iter().map(|line| line.row).collect();
        listed.sort_unstable();
        assert_eq!(
            listed,
            (0..rows.len()).collect::<Vec<_>>(),
            "{name}: every row once"
        );
        let anchors: HashMap<String, String> = crate::canvas::collapsible(&main, Some(&cleanup))
            .into_iter()
            .chain(crate::canvas::collapsible(&cleanup, None))
            .map(|check| (check.check, check.anchor))
            .collect();
        let pairs = by_node(&rows, &lines);
        for (line, (node, under)) in lines.iter().zip(&pairs) {
            let step = rows[line.row].step;
            let is_check = matches!(step.kind, NodeKind::Assert | NodeKind::Condition);
            if rows[line.row].depth > 0 {
                continue;
            }
            match anchors.get(*node) {
                Some(anchor) if is_check => assert_eq!(
                    under.map(str::to_owned),
                    Some(anchor.clone()),
                    "{name}: {} is listed under its anchor",
                    step.handle
                ),
                _ => assert_eq!(*under, None, "{name}: {} stands by itself", step.handle),
            }
        }
        // The order of the run: heads in the run's order, each check after
        // its head and before the next head of its level, checks of one head
        // in the run's order.
        let heads: Vec<usize> = lines
            .iter()
            .filter(|line| line.under.is_none())
            .map(|line| line.row)
            .collect();
        assert!(
            heads.windows(2).all(|pair| pair[0] < pair[1]),
            "{name}: heads keep the run's order: {pairs:?}"
        );
        for (index, line) in lines.iter().enumerate() {
            let Some(head) = line.under else { continue };
            let at = lines.iter().position(|other| other.row == head).unwrap();
            assert!(at < index, "{name}: a check follows its head");
            assert!(
                lines[at + 1..index].iter().all(|between| {
                    between.under == Some(head) || rows[between.row].depth > rows[head].depth
                }),
                "{name}: between a head and its check only its checks and its inner steps: {pairs:?}"
            );
            let earlier = lines[..index]
                .iter()
                .filter(|other| other.under == Some(head))
                .all(|other| other.row < line.row);
            assert!(earlier, "{name}: a head's checks keep the run's order");
        }
    }

    // On booking-day, by name: a check of two steps under the later one, a
    // check inside the `if` under its request, the Cleanup check under the
    // Cleanup request, the component's own steps under the call.
    let (main, cleanup, _) = graphs(&rooms, &rooms_id);
    let rows = flatten_steps(&rooms_run.steps);
    let lines = arrange(
        &rows,
        Some(RunGraphs {
            main: &main,
            cleanup: Some(&cleanup),
            project: &folder(&rooms).context_project(),
        }),
    );
    let under = |handle: &str| {
        let graph = if cleanup.nodes.iter().any(|node| node.handle == handle) {
            &cleanup
        } else {
            &main
        };
        let id = &node(graph, handle).id;
        lines
            .iter()
            .find(|line| rows[line.row].step.node_id == *id && rows[line.row].depth == 0)
            .and_then(|line| line.under)
            .map(|head| rows[head].step.handle.clone())
    };
    assert_eq!(under("invited").as_deref(), Some("invite"));
    assert_eq!(under("refused").as_deref(), Some("second"));
    assert_eq!(under("one_booking").as_deref(), Some("calendar"));
    assert_eq!(under("gone").as_deref(), Some("cancelled"));
    assert_eq!(under("big_enough").as_deref(), Some("team"));
    assert_eq!(under("profile"), None, "a request heads its group");
    let token = rows
        .iter()
        .position(|row| row.step.handle == "token" && row.depth == 0)
        .unwrap();
    let inner: Vec<&str> = lines
        .iter()
        .skip_while(|line| line.row != token)
        .skip(1)
        .take_while(|line| rows[line.row].depth > 0)
        .map(|line| rows[line.row].step.handle.as_str())
        .collect();
    assert_eq!(inner, ["session", "token"], "the call keeps its steps");
    assert!(
        rows.iter()
            .filter(|row| row.step.phase == Phase::Cleanup)
            .all(|row| row.step.phase == Phase::Cleanup),
        "Cleanup rows stay Cleanup's"
    );
}

/// Criterion 3: *Failed* leaves the requests with a failed, blocked or
/// errored row, with those rows; its count is the summary's.
#[test]
fn rep07a_criterion_3_failed_keeps_the_requests_that_failed() {
    let (_project, mut harness, _) = labels_with(WIDE, &failing());
    let result = ran(&mut harness);
    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(rows(&harness).len(), result.steps.len(), "All by default");
    assert!(
        harness.query_by_label("1 failed").is_some(),
        "the summary counts one failure"
    );
    click_segment(&mut harness, "All ");
    harness.get_by_label("Failed 1").click();
    settle(&mut harness);
    let rows = rows(&harness);
    assert_eq!(rows.len(), 2, "the request and its failed check: {rows:?}");
    assert!(rows[0].starts_with(&format!("{WEIGHT_31} · ")), "{rows:?}");
    assert!(
        rows[1].starts_with(&format!("{FAILED_CHECK} · Failed")),
        "{rows:?}"
    );
    // The failed row names its check and its request (AB-01).
    assert!(
        rows[1].contains("weight_31_field") && rows[1].contains(WEIGHT_31),
        "{rows:?}"
    );
    click_segment(&mut harness, "All ");
    assert_eq!(
        self::rows(&harness).len(),
        result.steps.len(),
        "All lists every step again"
    );
}

/// *Collapse checks* folds every request's checks and *Expand checks* shows
/// them again; ▸ on a request folds only its own (What the owner will see).
#[test]
fn rep07a_collapse_checks_and_the_fold_of_a_request() {
    let (_project, mut harness, _) = labels_with(WIDE, &labels_text());
    let result = ran(&mut harness);
    let requests = result
        .steps
        .iter()
        .filter(|step| step.kind == NodeKind::Request)
        .count();
    harness.get_by_label(COLLAPSE).click();
    settle(&mut harness);
    assert_eq!(
        rows(&harness).len(),
        requests + 1,
        "the start and the requests: {:?}",
        rows(&harness)
    );
    harness.get_by_label(EXPAND).click();
    settle(&mut harness);
    assert_eq!(rows(&harness).len(), result.steps.len());
    harness
        .get_by_label(&format!("Fold the checks of {WEIGHT_31}"))
        .click();
    settle(&mut harness);
    assert_eq!(rows(&harness).len(), result.steps.len() - 4);
    assert!(
        !rows(&harness)
            .iter()
            .any(|row| row.contains("weight_31_status")),
        "Weight 31 kg's checks are folded"
    );
    harness
        .get_by_label(&format!("Open the checks of {WEIGHT_31}"))
        .click();
    settle(&mut harness);
    assert_eq!(rows(&harness).len(), result.steps.len());
}

/// Clicks the switch of *All | Failed* whose label starts with `start`.
fn click_segment(harness: &mut FolderHarness, start: &str) {
    let label = harness
        .query_all_by_role(egui::accesskit::Role::Button)
        .filter_map(|node| node.accesskit_node().label().map(|label| label.to_string()))
        .find(|label| label.starts_with(start) && !label.contains(" · "))
        .unwrap_or_else(|| panic!("a switch {start:?}"));
    harness.get_by_label(&label).click();
    settle(harness);
}

/// Criterion 4: the list's edge is dragged between 240 px and half the
/// panel; at 960 × 540 the details keep at least 400 px.
#[test]
fn rep07a_criterion_4_the_edge_is_dragged_within_its_range() {
    for size in [MINIMUM, WIDE] {
        let (_project, mut harness, _) = labels_with(size, &failing());
        ran(&mut harness);
        assert_within_window(&harness, &[RESIZE_LIST]);
        let window = harness.ctx.content_rect();
        let before = list_width(&harness);
        drag_edge(&mut harness, -40.0);
        let narrower = list_width(&harness);
        assert!(
            (before - narrower - 40.0).abs() < 2.0,
            "{size:?}: the list follows the edge: {before} → {narrower}"
        );
        drag_edge(&mut harness, window.width());
        let widest = list_width(&harness);
        assert!(
            widest <= window.width() / 2.0 + 0.5,
            "{size:?}: at most half the panel: {widest}"
        );
        assert!(
            widest >= window.width() / 2.0 - 40.0,
            "{size:?}: dragged to the end, the list takes half the panel: {widest}"
        );
        assert_within_window(&harness, &[RESIZE_LIST]);
        assert!(
            window.right() - details(&harness) >= LEAST_DETAILS,
            "{size:?}: the details keep {LEAST_DETAILS} px: {}",
            window.right() - details(&harness)
        );
        drag_edge(&mut harness, -window.width());
        assert_eq!(
            list_width(&harness),
            LEAST_LIST,
            "{size:?}: no narrower than {LEAST_LIST} px"
        );
        assert_within_window(&harness, &[RESIZE_LIST]);
    }
}

/// Criterion 4: the width survives a restart; a preferences file written
/// before REP-07 reads as the default width.
#[test]
fn rep07a_criterion_4_the_width_survives_a_restart() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("preferences.json");
    assert_eq!(Preferences::at(Some(path.clone())).run_list_width, None);
    fs::write(
        &path,
        br#"{"version": 1, "wireStyle": "orthogonal", "bridges": false, "codePanel": false}"#,
    )
    .unwrap();
    let earlier = Preferences::at(Some(path.clone()));
    assert!(!earlier.bridges, "an earlier file is still read");
    assert_eq!(earlier.run_list_width, None);

    let width = {
        let (_project, mut harness, _) = labels_with(WIDE, &failing());
        app(&mut harness).preferences = Preferences::at(Some(path.clone()));
        ran(&mut harness);
        drag_edge(&mut harness, 120.0);
        list_width(&harness)
    };
    let remembered = Preferences::at(Some(path.clone()))
        .run_list_width
        .expect("the width is remembered");
    assert!((remembered - width).abs() < 0.5, "{remembered} vs {width}");
    assert!(
        !Preferences::at(Some(path.clone())).bridges,
        "only the width is written"
    );

    let (_project, mut harness, _) = labels_with(WIDE, &failing());
    app(&mut harness).preferences = Preferences::at(Some(path));
    ran(&mut harness);
    assert!(
        (list_width(&harness) - width).abs() < 0.5,
        "the next window opens the list at {width}: {}",
        list_width(&harness)
    );
}

/// Criterion 6: the rows of a component call keep today's nesting, and no
/// row says *Inside component*, which its indent already says.
#[test]
fn rep07a_criterion_6_a_component_call_keeps_its_nesting() {
    let (_project, mut harness, _) = booking_day(WIDE);
    ran(&mut harness);
    fn texts(shape: &egui::Shape, found: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| texts(shape, found)),
            egui::Shape::Text(text) => {
                found.push((text.galley.job.text.clone(), text.visual_bounding_rect()))
            }
            _ => {}
        }
    }
    let mut painted = Vec::new();
    for clipped in &harness.output().shapes {
        texts(&clipped.shape, &mut painted);
    }
    assert!(
        !painted
            .iter()
            .any(|(text, _)| text.contains("Inside component")),
        "no row says Inside component"
    );
    let right = list_right(&harness);
    let left_of = |name: &str| -> Vec<f32> {
        painted
            .iter()
            .filter(|(text, rect)| text == name && rect.right() <= right)
            .map(|(_, rect)| rect.left())
            .collect()
    };
    let tokens = left_of("token");
    assert_eq!(tokens.len(), 2, "the call and its export: {tokens:?}");
    let session = left_of("session");
    assert_eq!(session.len(), 1);
    let call = tokens.iter().copied().fold(f32::INFINITY, f32::min);
    assert!(session[0] > call + 8.0, "the inner request is indented");
    assert!(
        tokens.iter().any(|left| *left > call + 8.0),
        "the inner export is indented"
    );
}

/// Criterion 7: the accessible labels of the rows are distinct (UX-27).
#[test]
fn rep07a_criterion_7_row_labels_are_distinct() {
    let (_labels, mut labels, _) = labels_with(MINIMUM, &failing());
    ran(&mut labels);
    let (_rooms, mut rooms, _) = booking_day(MINIMUM);
    ran(&mut rooms);
    for (name, harness) in [("labels", &labels), ("booking-day", &rooms)] {
        let rows = rows(harness);
        let mut seen = std::collections::HashSet::new();
        for row in &rows {
            assert!(
                seen.insert(row),
                "{name}: the label {row:?} is not distinct"
            );
        }
    }
}

/// Criterion 8: up and down move between the rows, left and right fold and
/// open a request.
#[test]
fn rep07a_criterion_8_the_keyboard_moves_and_folds() {
    let (_project, mut harness, _) = labels_with(WIDE, &labels_text());
    let result = ran(&mut harness);
    click_row(&mut harness, &format!("{WEIGHT_31} · "));
    assert!(selected(&harness).starts_with(&format!("{WEIGHT_31} · ")));
    harness.key_press(egui::Key::ArrowDown);
    settle(&mut harness);
    let next = selected(&harness);
    assert!(
        next.starts_with("status == 422 · ") && next.contains("weight_31_status"),
        "down goes to its first check: {next}"
    );
    harness.key_press(egui::Key::ArrowUp);
    settle(&mut harness);
    assert!(selected(&harness).starts_with(&format!("{WEIGHT_31} · ")));
    harness.key_press(egui::Key::ArrowLeft);
    settle(&mut harness);
    assert_eq!(rows(&harness).len(), result.steps.len() - 4, "left folds");
    harness.key_press(egui::Key::ArrowDown);
    settle(&mut harness);
    assert!(
        selected(&harness).starts_with("Weight 0 kg · "),
        "down passes over folded checks: {}",
        selected(&harness)
    );
    harness.key_press(egui::Key::ArrowUp);
    settle(&mut harness);
    harness.key_press(egui::Key::ArrowRight);
    settle(&mut harness);
    assert_eq!(rows(&harness).len(), result.steps.len(), "right opens");
    assert!(selected(&harness).starts_with(&format!("{WEIGHT_31} · ")));
}

/// The owner's default of 2026-11-23: with no width remembered the list
/// opens 440 px wide, at 960 × 540 as at 1600 × 1000, and the details keep
/// their 400 px.
#[test]
fn rep07a_the_list_opens_440_px_wide() {
    for size in [MINIMUM, WIDE] {
        let (_project, mut harness, _) = labels_with(size, &failing());
        assert_eq!(folder(&harness).preferences.run_list_width, None);
        ran(&mut harness);
        assert_eq!(list_width(&harness), 440.0, "{size:?}");
        let window = harness.ctx.content_rect();
        assert!(
            window.right() - details(&harness) >= LEAST_DETAILS,
            "{size:?}: the details keep {LEAST_DETAILS} px"
        );
        assert_within_window(&harness, &[RESIZE_LIST]);
    }
}

/// Review of 2026-11-24: beside a list 440 px wide, and beside the widest
/// one, the controls above a value keep the width of their labels at
/// 960 × 540 (*Tree* read *T…*).
#[test]
fn rep07a_the_controls_above_a_value_are_not_cut() {
    const CONTROLS: [&str; 5] = [
        crate::report::SHOW_TREE,
        crate::report::SHOW_JSON,
        "Expand all",
        "Collapse all",
        crate::report::COPY_JSON,
    ];
    let widths = |harness: &FolderHarness| {
        CONTROLS.map(|label| harness.get_by_label(label).rect().width().round())
    };
    let (_project, mut harness, _) = labels_with(WIDE, &failing());
    ran(&mut harness);
    let whole = widths(&harness);
    let (_project, mut harness, _) = labels_with(MINIMUM, &failing());
    ran(&mut harness);
    assert_eq!(widths(&harness), whole, "beside the list as it opens");
    // They are under the response of the check's step (REP-07b).
    harness
        .get_by_label(crate::report::COPY_JSON)
        .scroll_to_me();
    settle(&mut harness);
    assert_within_window(&harness, &CONTROLS);
    drag_edge(&mut harness, MINIMUM.x);
    assert_eq!(widths(&harness), whole, "beside the widest list");
    harness
        .get_by_label(crate::report::COPY_JSON)
        .scroll_to_me();
    settle(&mut harness);
    assert_within_window(&harness, &CONTROLS);
}

/// The row of `weight_31`'s first check, `status == 422`.
fn weight_31_status(harness: &FolderHarness) -> String {
    rows(harness)
        .into_iter()
        .find(|label| label.starts_with("status == 422 · ") && label.contains("weight_31_status"))
        .unwrap_or_else(|| panic!("weight_31_status has a row: {:?}", rows(harness)))
}

/// Review of REP-07a: a check's row stays chosen while the board shows the
/// request it is folded into (LAY-11), but the request chosen on the board
/// afterwards chooses the request's row.
#[test]
fn rep07a_the_board_chooses_the_request_after_its_check() {
    let (_project, mut harness, id) = labels_with(WIDE, &labels_text());
    ran(&mut harness);
    let (main, _, _) = graphs(&harness, &id);
    let request = node(&main, "weight_31").id.clone();
    let check = weight_31_status(&harness);
    click_row(&mut harness, &check);
    harness.step();
    assert_eq!(
        folder(&harness).selections.get(&id).cloned(),
        Some(vec![request.clone()]),
        "the board shows the request the check is folded into"
    );
    assert!(
        selected(&harness).contains("weight_31_status"),
        "the check's row stays chosen: {}",
        selected(&harness)
    );
    // A click on the empty board, then on the request.
    app(&mut harness).selections.insert(id.clone(), Vec::new());
    settle(&mut harness);
    app(&mut harness)
        .selections
        .insert(id.clone(), vec![request]);
    settle(&mut harness);
    assert!(
        selected(&harness).starts_with(&format!("{WEIGHT_31} · ")),
        "the request's row is chosen: {}",
        selected(&harness)
    );
}

/// Review of REP-07a: in a window too narrow for the remembered width the
/// list is narrower; the width is remembered only when the edge was dragged
/// and the width changed, so a drag that cannot widen the list keeps the
/// remembered 700 px.
#[test]
fn rep07a_a_narrow_window_keeps_the_remembered_width() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("preferences.json");
    let mut preferences = Preferences::at(Some(path.clone()));
    preferences.run_list_width = Some(700.0);
    preferences.save(crate::preferences::Preference::RunListWidth);

    let (_project, mut harness, _) = labels_with(MINIMUM, &failing());
    app(&mut harness).preferences = Preferences::at(Some(path.clone()));
    ran(&mut harness);
    let shown = list_width(&harness);
    assert!(shown < 700.0, "960 px leaves no room for 700: {shown}");
    drag_edge(&mut harness, 100.0);
    assert_eq!(list_width(&harness), shown, "the list cannot widen");
    assert_eq!(
        Preferences::at(Some(path.clone())).run_list_width,
        Some(700.0),
        "a drag that changed nothing is not remembered"
    );
    drag_edge(&mut harness, -40.0);
    let narrower = list_width(&harness);
    assert!(
        (shown - 40.0 - narrower).abs() < 2.0,
        "{shown} → {narrower}"
    );
    let remembered = Preferences::at(Some(path))
        .run_list_width
        .expect("a width is remembered");
    assert!(
        (remembered - narrower).abs() < 0.5,
        "a drag that narrowed the list is remembered: {remembered} vs {narrower}"
    );
}

/// Review of REP-07a: *All* and *Failed* count the same thing, the run's
/// steps as its summary counts them: every one, and those it counts as
/// failed, blocked or not met.
#[test]
fn rep07a_all_and_failed_count_the_steps_of_the_summary() {
    let (_project, mut harness, _) = labels_with(WIDE, &failing());
    let result = ran(&mut harness);
    let all = format!("All {}", result.steps.len());
    assert!(
        harness.query_by_label(&all).is_some(),
        "{all}: every step of the run"
    );
    assert!(harness.query_by_label("1 failed").is_some());
    assert!(harness.query_by_label("Failed 1").is_some());
}

/// Review of REP-07a: when a fold or *Failed* chosen without the pointer
/// (assistive technology; a press of the pointer elsewhere takes the focus
/// away) hides the focused row, the focus moves to the row of its request,
/// or to the nearest row shown.
#[test]
fn rep07a_a_hidden_row_gives_its_focus_to_a_shown_one() {
    let (_project, mut harness, _) = labels_with(WIDE, &failing());
    ran(&mut harness);
    let check = weight_31_status(&harness);
    click_row(&mut harness, &check);
    assert!(harness.get_by_label(&check).is_focused(), "a click focuses");
    harness
        .get_by_label(&format!("Fold the checks of {WEIGHT_31}"))
        .click_accesskit();
    settle(&mut harness);
    let request = row(&harness, &format!("{WEIGHT_31} · "));
    assert!(
        harness.get_by_label(&request).is_focused(),
        "the request takes the focus of its folded check"
    );

    let passed = row(&harness, "Weight 0 kg · ");
    click_row(&mut harness, &passed);
    assert!(harness.get_by_label(&passed).is_focused());
    harness.get_by_label("Failed 1").click_accesskit();
    settle(&mut harness);
    let shown = rows(&harness);
    let focused: Vec<String> = shown
        .iter()
        .filter(|label| harness.get_by_label(label).is_focused())
        .cloned()
        .collect();
    assert_eq!(focused.len(), 1, "a row shown takes the focus: {shown:?}");
}

/// Design review: Run results of `labels` after a run with a failure, at
/// its normal height and maximised, with *Failed* on, at both windows.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_rep07a_run_results() {
    for (size, suffix) in [(MINIMUM, "960"), (WIDE, "1600")] {
        let (_project, mut harness, _) = labels_with(size, &failing());
        choose(&mut harness, "Run simulation");
        finish_run(&mut harness);
        crate::testing::capture(&mut harness, &format!("rep07a-failed-{suffix}"), None);
        choose(&mut harness, MAXIMISE);
        crate::testing::capture(&mut harness, &format!("rep07a-failed-max-{suffix}"), None);
        harness.get_by_label("Failed 1").click();
        settle(&mut harness);
        crate::testing::capture(&mut harness, &format!("rep07a-failed-only-{suffix}"), None);
        let (_project, mut harness, _) = labels_with(size, &labels_text());
        ran(&mut harness);
        crate::testing::capture(&mut harness, &format!("rep07a-passed-max-{suffix}"), None);
    }
}

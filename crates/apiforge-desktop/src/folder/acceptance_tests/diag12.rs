//! DIAG-12 (#533): a field of a response that a check already reads is
//! marked, in the Inspector's *Output data* and in Run results. Decided by
//! the owner on 2026-11-23: the ✓ is filled (outlined on a folded field with
//! a checked field inside); in Run results *Check* reads *2 checks* or
//! *1 failed* in the colour of the outcome; the menu of ✓ lists the field's
//! checks first, then *Add a check*; checks only, on the field the check
//! names. On the board's fixture `cart`.
use super::mcp08b::contains;
use super::*;
use crate::inspector::{CheckIndex, DrawnMark, MarkTone, Marks, Outcomes};
use crate::testing::MINIMUM;
use apiforge_core::format::{Graph, Node};
use apiforge_core::model::NodeKind;
use egui_kittest::kittest::NodeT;

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";

/// The board's response: a text, a number, a boolean, an object, a list and
/// a null.
const SAMPLE: &str = r#"    sample 201 { id: "c-1", total: 172.5, paid: false, owner: { name: "Ann", email: "ann@example.com" }, items: [{ sku: "coffee", price: 12.5 }], coupon: null }
"#;

/// Six checks of `cart`: two on `total`, one that fails on `paid`, one on
/// the nested `owner.name`, one on the list `items` and one on the status.
const CHECKS: &str = r#"  status_created = assert cart.status == 201
  total_number = assert cart.body.total is "number"
  total_value = assert cart.body.total == 172.5
  has_items = assert nonempty cart.body.items
  owner_name = assert cart.body.owner.name == "Ann"
  paid_true = assert cart.body.paid == true
"#;

/// The scenario `cart` with `checks`, and `cleanup` when it is not empty.
fn source(checks: &str, cleanup: &str) -> String {
    let cleanup = if cleanup.is_empty() {
        String::new()
    } else {
        format!("  cleanup {{\n{cleanup}  }}\n")
    };
    format!(
        "scenario \"A cart\" {{\n  begin = start\n  cart = POST \"/carts\" {{\n{SAMPLE}  }}\n{checks}{cleanup}}}\n"
    )
}

/// What each check of [`CHECKS`] reads as DIAG-02 writes it under `cart`.
const TOTAL: [&str; 2] = ["total is \"number\"", "total == 172.5"];
const PAID: &str = "paid == true";
const ITEMS: &str = "nonempty items";
const OWNER_NAME: &str = "owner.name == \"Ann\"";
const STATUS: &str = "status == 201";

const CHECK: &str = "Add a check";
const RESULTS_CHECK: &str = "Check";
const MENU: &str = "Add a check menu";
/// The mark alone, where the results take no check (review round).
const CHECKS_OF_FIELD: &str = "Checks of this field";

/// `text` as the demo shop's scenario, on its board at `size`, never run.
fn opened(size: egui::Vec2, text: &str) -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), text).unwrap();
    opened_in(project, size)
}

fn opened_in(
    project: tempfile::TempDir,
    size: egui::Vec2,
) -> (tempfile::TempDir, FolderHarness, String) {
    let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let id = only(&harness, DocumentKind::Scenario);
    (project, harness, id)
}

/// A simulation of the scenario, finished: `paid_true` fails.
fn run(harness: &mut FolderHarness) -> RunResult {
    choose(harness, "Run simulation");
    finish_run(harness)
}

fn graph(harness: &FolderHarness, id: &str) -> Graph {
    let snapshot = folder(harness).snapshot(id).unwrap();
    graph_of(snapshot.model.as_ref().unwrap(), false).clone()
}

fn cleanup_graph(harness: &FolderHarness, id: &str) -> Graph {
    let snapshot = folder(harness).snapshot(id).unwrap();
    graph_of(snapshot.model.as_ref().unwrap(), true).clone()
}

fn node<'a>(graph: &'a Graph, handle: &str) -> &'a Node {
    graph
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("a step {handle}"))
}

/// The steps `after` has and `before` has not.
fn added<'a>(before: &Graph, after: &'a Graph) -> Vec<&'a Node> {
    after
        .nodes
        .iter()
        .filter(|node| before.nodes.iter().all(|old| old.id != node.id))
        .collect()
}

/// The step `handle` selected, the Inspector on *Output data*.
fn output_data(harness: &mut FolderHarness, id: &str, handle: &str) {
    let step = node(&graph(harness, id), handle).id.clone();
    app(harness).selections.insert(id.into(), vec![step]);
    settle(harness);
    super::inspector_tab(harness, "Output data")
        .expect("the Inspector has an Output data tab")
        .click();
    settle(harness);
}

/// The Inspector's left edge.
fn edge(harness: &FolderHarness) -> f32 {
    harness.get_by_label("Resize inspector").rect().left()
}

/// The name `field` on its row right of `left`.
fn name_right_of(harness: &FolderHarness, field: &str, left: f32) -> egui::Rect {
    harness
        .query_all_by_label(field)
        .map(|node| node.rect())
        .find(|rect| rect.left() > left)
        .unwrap_or_else(|| panic!("a row named {field} is shown"))
}

/// The widget labelled `label` on the row whose name is at `row`.
fn on_row<'a>(
    harness: &'a FolderHarness,
    row: egui::Rect,
    label: &'a str,
) -> Option<egui_kittest::Node<'a>> {
    harness
        .query_all_by_label(label)
        .filter(|node| (node.rect().center().y - row.center().y).abs() < 13.0)
        .filter(|node| node.rect().left() >= row.right())
        .min_by(|a, b| {
            let a = (a.rect().center().y - row.center().y).abs();
            let b = (b.rect().center().y - row.center().y).abs();
            a.total_cmp(&b)
        })
}

/// The ✓ on the Inspector's row of `field`.
fn check_on<'a>(harness: &'a FolderHarness, field: &str) -> egui_kittest::Node<'a> {
    let row = name_right_of(harness, field, edge(harness));
    on_row(harness, row, CHECK).unwrap_or_else(|| panic!("the row {field} has {CHECK}"))
}

/// What the mark on the Inspector's row of `field` says to assistive
/// technology, one check a line; `None` where the row carries no mark.
fn mark(harness: &FolderHarness, field: &str) -> Option<String> {
    check_on(harness, field).accesskit_node().description()
}

/// The marks drawn in the next frame.
fn drawn(harness: &mut FolderHarness) -> Vec<DrawnMark> {
    crate::inspector::debug_forget_marks();
    harness.step();
    crate::inspector::debug_marks()
}

/// The mark drawn on the row of `path` of `port`, in Run results or the
/// Inspector.
fn drawn_on<'a>(
    marks: &'a [DrawnMark],
    results: bool,
    port: &str,
    path: &str,
) -> Option<&'a DrawnMark> {
    marks.iter().find(|mark| {
        mark.shown && mark.results == results && mark.port == port && mark.path == path
    })
}

/// The rows of `cart` marked and not marked by [`CHECKS`] (criterion 1).
fn assert_marked_rows(harness: &FolderHarness, when: &str) {
    for field in ["total", "paid", "items", "HTTP status"] {
        assert!(
            mark(harness, field).is_some(),
            "{field} carries the mark {when}"
        );
    }
    for field in ["id", "coupon"] {
        assert_eq!(mark(harness, field), None, "{field} carries no mark {when}");
    }
}

/// Opens `owner` in the Inspector.
fn open_owner(harness: &mut FolderHarness) {
    let row = name_right_of(harness, "owner", edge(harness));
    let left = row.left() - 40.0;
    harness
        .query_all_by_label("Expand field")
        .find(|node| {
            (node.rect().center().y - row.center().y).abs() < 13.0 && node.rect().left() > left
        })
        .expect("owner can be opened")
        .click();
    harness.remove_cursor();
    settle(harness);
}

/// Criterion 1: in *Output data* of `cart` the rows `total`, `paid`,
/// `items`, `owner.name` and *HTTP status* carry the mark and `id`,
/// `coupon` do not; before a run and after it.
#[test]
fn diag12_criterion_1_checked_rows_carry_the_mark() {
    for size in [WIDE, MINIMUM] {
        let (_project, mut harness, id) = opened(size, &source(CHECKS, ""));
        output_data(&mut harness, &id, "cart");
        assert_marked_rows(&harness, "before a run");
        open_owner(&mut harness);
        assert!(
            mark(&harness, "name").is_some(),
            "owner.name carries the mark"
        );
        assert_eq!(mark(&harness, "email"), None, "owner.email carries none");

        run(&mut harness);
        output_data(&mut harness, &id, "cart");
        assert_marked_rows(&harness, "after a run");
        assert!(
            mark(&harness, "name").is_some(),
            "owner.name carries the mark after a run"
        );
        let marks = drawn(&mut harness);
        let total = drawn_on(&marks, false, "body", "total").expect("total is marked");
        assert!(!total.inside, "total's own mark is filled");
        assert_eq!(
            total.tone,
            MarkTone::Plain,
            "the Inspector's mark says that a check exists, not how it ended"
        );
    }
}

/// Criterion 2: the mark's hover and accessible description list the
/// checks in the text of `canvas::check_text` (DIAG-02), one a line. The
/// accessible name stays *Add a check* (DIAG-10 criterion 6).
#[test]
fn diag12_criterion_2_the_mark_names_the_checks() {
    let (_project, mut harness, id) = opened(WIDE, &source(CHECKS, ""));
    output_data(&mut harness, &id, "cart");
    let graph = graph(&harness, &id);
    let project = folder(&harness).context_project();
    // The texts are DIAG-02's, from the check's node.
    for (handle, expected) in [
        ("total_number", TOTAL[0]),
        ("total_value", TOTAL[1]),
        ("paid_true", PAID),
        ("has_items", ITEMS),
        ("owner_name", OWNER_NAME),
        ("status_created", STATUS),
    ] {
        let check = node(&graph, handle);
        let scope = apiforge_core::template_text::Scope {
            nodes: &graph.nodes,
            project: &project,
            request: Some(check),
        };
        let own = crate::canvas::own_step(check, &graph);
        assert_eq!(
            crate::canvas::check_text(check, scope, &graph, own).text(),
            expected,
            "{handle} reads as DIAG-02 writes it"
        );
    }
    for (field, lines) in [
        ("total", &TOTAL[..]),
        ("paid", &[PAID][..]),
        ("items", &[ITEMS][..]),
        ("HTTP status", &[STATUS][..]),
    ] {
        let said = mark(&harness, field).unwrap_or_default();
        assert_eq!(
            said.lines().collect::<Vec<_>>(),
            lines,
            "the mark on {field} lists its checks, one a line"
        );
        assert_eq!(
            check_on(&harness, field)
                .accesskit_node()
                .label()
                .as_deref(),
            Some(CHECK),
            "the ✓ on {field} keeps its name"
        );
    }
    let shows = |harness: &FolderHarness, text: &str| {
        harness
            .query_all_by_label_contains(text)
            .any(|node| node.accesskit_node().role() != egui::accesskit::Role::Button)
    };
    assert!(
        crate::testing::hover_until(
            &mut harness,
            |harness| Some(check_on(harness, "total").rect()),
            |harness| shows(harness, TOTAL[0]) && shows(harness, TOTAL[1]),
        ),
        "pointing at the mark on total names its two checks"
    );
}

/// Criterion 3: `owner` folded carries the outlined mark and the hover
/// *1 check inside*; opened, it carries none and `name` carries the mark.
#[test]
fn diag12_criterion_3_a_folded_field_says_what_is_inside() {
    let (_project, mut harness, id) = opened(WIDE, &source(CHECKS, ""));
    output_data(&mut harness, &id, "cart");
    assert_eq!(
        mark(&harness, "owner").as_deref(),
        Some("1 check inside"),
        "owner folded says what it holds"
    );
    let marks = drawn(&mut harness);
    let owner = drawn_on(&marks, false, "body", "owner").expect("owner folded is marked");
    assert!(owner.inside, "owner's mark is the outlined one");
    assert!(
        crate::testing::hover_until(
            &mut harness,
            |harness| Some(check_on(harness, "owner").rect()),
            |harness| harness
                .query_all_by_label_contains("1 check inside")
                .any(|node| node.accesskit_node().role() != egui::accesskit::Role::Button),
        ),
        "pointing at owner's mark reads 1 check inside"
    );
    harness.remove_cursor();
    settle(&mut harness);

    open_owner(&mut harness);
    assert_eq!(mark(&harness, "owner"), None, "owner opened carries none");
    assert_eq!(
        mark(&harness, "name").as_deref(),
        Some(OWNER_NAME),
        "name carries the mark of owner.name"
    );
    let marks = drawn(&mut harness);
    assert!(drawn_on(&marks, false, "body", "owner").is_none());
    assert!(
        !drawn_on(&marks, false, "body", "owner.name")
            .unwrap()
            .inside
    );
    // `nonempty items` marks items, not its children.
    let row = name_right_of(&harness, "items", edge(&harness));
    harness
        .query_all_by_label("Expand field")
        .find(|node| (node.rect().center().y - row.center().y).abs() < 13.0)
        .expect("items can be opened")
        .click();
    harness.remove_cursor();
    settle(&mut harness);
    assert_eq!(mark(&harness, "items").as_deref(), Some(ITEMS));
    assert_eq!(
        mark(&harness, "[0]"),
        None,
        "the item of items is not marked"
    );
}

/// The node in the open menu labelled `label`.
fn in_menu<'a>(harness: &'a FolderHarness, label: &'a str) -> Option<egui_kittest::Node<'a>> {
    let menu = harness.query_by_label(MENU)?.rect();
    harness
        .query_all_by_label(label)
        .find(|node| menu.contains_rect(node.rect().shrink(1.0)))
}

/// Whether choosing `check` from a menu selected it: the check, or the step
/// it is folded into with its *Checks* list open (LAY-11).
fn assert_selected(harness: &FolderHarness, id: &str, check: &str, subject: &str) {
    let graph = graph(harness, id);
    let selected = folder(harness).selections.get(id).cloned();
    let check_id = node(&graph, check).id.clone();
    let subject_id = node(&graph, subject).id.clone();
    let listed = harness
        .query_all_by_label_contains(&format!("{check} — "))
        .next()
        .is_some();
    assert!(
        selected.as_deref() == Some(&[check_id][..])
            || (selected.as_deref() == Some(&[subject_id][..]) && listed),
        "choosing {check} selects it: {selected:?}"
    );
}

/// Criterion 4: the ✓ of a marked row opens the menu with the field's
/// checks above *Add a check*; choosing one selects that check; adding a
/// second check from the same menu works.
#[test]
fn diag12_criterion_4_the_menu_lists_the_checks_first() {
    let (_project, mut harness, id) = opened(WIDE, &source(CHECKS, ""));
    output_data(&mut harness, &id, "cart");
    check_on(&harness, "total").click();
    settle(&mut harness);
    assert!(has_label(&harness, MENU), "the ✓ on total opens its menu");
    let first = in_menu(&harness, TOTAL[0]).expect("the menu lists total is \"number\"");
    let second = in_menu(&harness, TOTAL[1]).expect("the menu lists total == 172.5");
    let add = in_menu(&harness, "ADD A CHECK").expect("the menu still adds a check");
    let offered = in_menu(&harness, "total exists").expect("the menu offers total exists");
    assert!(
        first.rect().bottom() <= second.rect().top() + 1.0
            && second.rect().bottom() <= add.rect().top()
            && add.rect().bottom() <= offered.rect().top() + 1.0,
        "the checks stand above Add a check"
    );
    let before = graph(&harness, &id);
    in_menu(&harness, TOTAL[1]).unwrap().click();
    harness.remove_cursor();
    settle(&mut harness);
    assert_eq!(
        graph(&harness, &id).nodes.len(),
        before.nodes.len(),
        "choosing a check adds nothing"
    );
    assert_selected(&harness, &id, "total_value", "cart");

    // A second check from the same menu.
    output_data(&mut harness, &id, "cart");
    let before = graph(&harness, &id);
    check_on(&harness, "total").click();
    settle(&mut harness);
    in_menu(&harness, "total exists")
        .expect("the menu offers total exists")
        .click();
    harness.remove_cursor();
    settle(&mut harness);
    let after = graph(&harness, &id);
    let new = added(&before, &after);
    assert_eq!(new.len(), 1, "total exists adds one check: {new:?}");
    assert_eq!(new[0].kind, NodeKind::Assert);
    output_data(&mut harness, &id, "cart");
    assert_eq!(
        mark(&harness, "total").unwrap_or_default().lines().count(),
        3,
        "total now lists three checks"
    );
}

/// Run results, maximised, on the step `step`.
fn results_on(harness: &mut FolderHarness, step: &str) {
    if harness.query_by_label("Maximise run results").is_some() {
        choose(harness, "Maximise run results");
    }
    // The step's name may stand on more than one node: the first button
    // is its row.
    let row = |harness: &FolderHarness| {
        harness
            .query_all_by_label_contains(&format!("{step} · "))
            .find(|node| node.accesskit_node().role() == egui::accesskit::Role::Button)
            .map(|node| node.rect().center())
            .unwrap_or_else(|| panic!("Run results list the step {step}"))
    };
    harness
        .query_all_by_label_contains(&format!("{step} · "))
        .find(|node| node.accesskit_node().role() == egui::accesskit::Role::Button)
        .unwrap()
        .scroll_to_me();
    settle(harness);
    let at = row(harness);
    harness.hover_at(at);
    harness.drag_at(at);
    harness.drop_at(at);
    harness.remove_cursor();
    settle(harness);
}

/// Where the selected step's details start: the left of its first tab.
fn details(harness: &FolderHarness) -> f32 {
    harness
        .query_all_by_label_contains("Outputs  ")
        .map(|node| node.rect().left())
        .next()
        .expect("the step's details show their Outputs tab")
}

/// *Check* on the Run results row of `field`.
fn results_check<'a>(harness: &'a FolderHarness, field: &str) -> egui_kittest::Node<'a> {
    let row = name_right_of(harness, field, details(harness));
    on_row(harness, row, RESULTS_CHECK)
        .unwrap_or_else(|| panic!("the row {field} has {RESULTS_CHECK}"))
}

/// Criterion 5: in Run results the mark is green where every check of the
/// field passed, red where one failed (and reads *1 failed*), neutral where
/// none ran; where *Check* is an icon the mark is the icon's.
#[test]
fn diag12_criterion_5_run_results_take_the_outcome() {
    let (_project, mut harness, id) = opened(WIDE, &source(CHECKS, ""));
    let report = run(&mut harness);
    assert_eq!(report.status, RunStatus::Failed, "paid == true fails");
    results_on(&mut harness, "cart");
    let marks = drawn(&mut harness);
    let total = drawn_on(&marks, true, "body", "total").expect("total is marked");
    assert_eq!(
        (total.tone, total.words.as_str()),
        (MarkTone::Passed, "2 checks")
    );
    let paid = drawn_on(&marks, true, "body", "paid").expect("paid is marked");
    assert_eq!(
        (paid.tone, paid.words.as_str()),
        (MarkTone::Failed, "1 failed")
    );
    let name = drawn_on(&marks, true, "body", "owner.name").expect("owner.name is marked");
    assert_eq!(
        (name.tone, name.words.as_str()),
        (MarkTone::Passed, "1 check")
    );
    let status = drawn_on(&marks, true, "status", "").expect("the status is marked");
    assert_eq!(status.tone, MarkTone::Passed);
    assert!(drawn_on(&marks, true, "body", "id").is_none(), "id is not");
    assert!(
        !total.icon && !paid.icon,
        "the words have room in a maximised panel"
    );
    // The words are *Check*'s: the control keeps its name and position.
    let row = name_right_of(&harness, "paid", details(&harness));
    let check = results_check(&harness, "paid");
    assert!(
        check.rect().left() >= row.right(),
        "the mark stands where Check does"
    );
    assert!(
        check
            .accesskit_node()
            .description()
            .unwrap_or_default()
            .lines()
            .next()
            .is_some_and(|line| line.starts_with(PAID)),
        "the failed check is named on paid's Check"
    );

    // A check added since the run has not run: neutral.
    let index = CheckIndex::build(
        &graph(&harness, &id),
        None,
        &folder(&harness).context_project(),
    );
    let mut unknown = report.clone();
    unknown.steps.retain(|step| step.handle != "total_value");
    let narrow = tree_drawn(
        &index,
        &unknown,
        &node(&graph(&harness, &id), "cart").id,
        &report,
        260.0,
    );
    let total = drawn_on(&narrow, true, "body", "total").expect("total is marked");
    assert_eq!(
        total.tone,
        MarkTone::NotRun,
        "neutral where a check of the field has not run"
    );
    // Where *Check* is an icon, the mark is the icon's.
    assert!(total.icon, "a 260 px panel shows Check as an icon");
    let paid = drawn_on(&narrow, true, "body", "paid").unwrap();
    assert_eq!((paid.tone, paid.icon), (MarkTone::Failed, true));
    let none_ran = {
        let mut empty = report.clone();
        empty.steps.retain(|step| step.handle == "cart");
        tree_drawn(
            &index,
            &empty,
            &node(&graph(&harness, &id), "cart").id,
            &report,
            600.0,
        )
    };
    for (path, tone) in [("total", MarkTone::NotRun), ("paid", MarkTone::NotRun)] {
        assert_eq!(
            drawn_on(&none_ran, true, "body", path).unwrap().tone,
            tone,
            "{path} is neutral where none ran"
        );
    }
}

/// Review round (2026-11-23): a stale report keeps its marks. After an
/// edit the run's marks still stand on *Check*, in the colour of the run;
/// their menu lists the field's checks, a check the run does not know
/// *Not run*, and offers neither *Add a check* nor *To variable*.
/// Choosing that check selects it on the board; the details pane keeps the
/// step whose field it was chosen on, rather than going blank.
#[test]
fn diag12_review_a_stale_report_keeps_its_marks() {
    let (_project, mut harness, id) = opened(WIDE, &source(CHECKS, ""));
    app(&mut harness).debug_draw_checks();
    run(&mut harness);
    // An edit after the run: a check of total the run does not know.
    let edited = source(
        &format!("{CHECKS}  total_exists = assert exists cart.body.total\n"),
        "",
    );
    app(&mut harness)
        .session
        .as_mut()
        .unwrap()
        .edit_source(&id, &edited, CODE)
        .unwrap();
    settle(&mut harness);
    assert!(
        folder(&harness).fresh_run(&id).is_none(),
        "the edit makes the report stale"
    );
    results_on(&mut harness, "cart");
    let marks = drawn(&mut harness);
    let total = drawn_on(&marks, true, "body", "total").expect("total keeps its mark");
    assert_eq!(
        (total.tone, total.words.as_str()),
        (MarkTone::NotRun, "3 checks"),
        "a check the run does not know is not run"
    );
    let paid = drawn_on(&marks, true, "body", "paid").expect("paid keeps its mark");
    assert_eq!(
        (paid.tone, paid.words.as_str()),
        (MarkTone::Failed, "1 failed")
    );
    assert!(
        drawn_on(&marks, true, "body", "id").is_none(),
        "id has no check"
    );

    // The menu lists the checks and adds nothing: the mark is named for
    // the checks, so no field offers *Check* (UX-01 criterion 2).
    let row = name_right_of(&harness, "total", details(&harness));
    assert!(
        on_row(&harness, row, "To variable").is_none(),
        "a stale report offers no variable"
    );
    assert!(
        on_row(&harness, row, RESULTS_CHECK).is_none(),
        "a stale report offers no Check"
    );
    on_row(&harness, row, CHECKS_OF_FIELD)
        .expect("the mark on total is named for its checks")
        .click();
    settle(&mut harness);
    assert!(has_label(&harness, MENU), "the mark opens its menu");
    for check in TOTAL {
        assert!(in_menu(&harness, check).is_some(), "the menu lists {check}");
    }
    assert!(
        in_menu(&harness, "ADD A CHECK").is_none() && in_menu(&harness, "total exists").is_none(),
        "a stale report adds no check"
    );
    let before = graph(&harness, &id);
    in_menu(&harness, "exists total")
        .expect("the menu lists the check added since the run")
        .click();
    harness.remove_cursor();
    settle(&mut harness);
    assert_eq!(graph(&harness, &id).nodes.len(), before.nodes.len());
    // Drawn as a card, the check itself is selected, not the step.
    assert_eq!(
        folder(&harness).selections.get(&id).cloned(),
        Some(vec![node(&graph(&harness, &id), "total_exists").id.clone()]),
        "choosing total_exists selects it on the board"
    );
    // The details pane keeps the step the menu was opened on.
    assert!(
        !contains(&harness, "Results will appear while the scenario runs."),
        "the details pane keeps cart's results"
    );
    assert!(
        name_right_of(&harness, "total", details(&harness)).left() > details(&harness),
        "cart's fields stay in the details pane"
    );
}

/// Review round (2026-11-23): where the board takes no edit, the marked ✓
/// in the Inspector stays enabled: its menu lists the field's checks, a
/// click selects one, and only *Add a check* is disabled.
#[test]
fn diag12_review_a_read_only_inspector_lists_the_checks() {
    let (_project, mut harness, id) = opened(WIDE, &source(CHECKS, ""));
    output_data(&mut harness, &id, "cart");
    let broken = format!("{}\n}}\n", source_of(&harness, &id));
    app(&mut harness)
        .session
        .as_mut()
        .unwrap()
        .edit_source(&id, &broken, CODE)
        .unwrap();
    settle(&mut harness);
    assert!(
        !folder(&harness).snapshot(&id).unwrap().editable,
        "the board takes no edit"
    );
    assert!(
        !check_on(&harness, "total").accesskit_node().is_disabled(),
        "the marked ✓ is enabled"
    );
    assert!(
        check_on(&harness, "id").accesskit_node().is_disabled(),
        "an unmarked ✓ stays disabled"
    );
    let before = graph(&harness, &id);
    check_on(&harness, "total").click();
    settle(&mut harness);
    assert!(has_label(&harness, MENU), "the marked ✓ opens its menu");
    let offered = in_menu(&harness, "total exists").expect("the menu shows Add a check");
    assert!(
        offered.accesskit_node().is_disabled(),
        "Add a check is disabled"
    );
    in_menu(&harness, TOTAL[1])
        .expect("the menu lists total == 172.5")
        .click();
    harness.remove_cursor();
    settle(&mut harness);
    assert_eq!(
        graph(&harness, &id).nodes.len(),
        before.nodes.len(),
        "choosing a check adds nothing"
    );
    assert_selected(&harness, &id, "total_value", "cart");
}

/// The marks Run results' tree of `cart`'s body draws at `width`, with the
/// outcomes of `outcomes`.
fn tree_drawn(
    index: &CheckIndex,
    outcomes: &RunResult,
    cart: &str,
    report: &RunResult,
    width: f32,
) -> Vec<DrawnMark> {
    let body = report
        .steps
        .iter()
        .find(|step| step.handle == "cart")
        .and_then(|step| step.outputs.get("body"))
        .expect("cart has a body")
        .clone();
    let context = egui::Context::default();
    crate::theme::apply(&context);
    crate::inspector::debug_forget_marks();
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width, 800.0),
            )),
            ..Default::default()
        },
        |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                crate::inspector::results_tree(
                    ui,
                    egui::Id::new("diag12-tree"),
                    cart,
                    "body",
                    &body,
                    crate::inspector::Results {
                        opening: crate::inspector::Opening::Leaves,
                        fields: true,
                        offers: true,
                        checked: &|_| false,
                        marks: Some(Marks {
                            index,
                            run: Some(&Outcomes::of(outcomes)),
                        }),
                    },
                );
            });
        },
    );
    output.textures_delta.clear();
    crate::inspector::debug_marks()
}

/// The demo shop with a component `checked` whose inner step's field a check
/// of its own reads, published as revision 1.
fn shop_with_checked_component() -> tempfile::TempDir {
    let project = crate::testing::shop_project();
    let mut session = apiforge_project::ProjectSession::open(project.path()).unwrap();
    let id = session
        .create_document(
            DocumentKind::Component,
            "components/checked.afs",
            "component checked \"Checked\" {\n  fetch = GET \"/things\" {\n    sample 200 { name: \"Ann\" }\n  }\n  has_name = assert exists fetch.body.name\n  export thing: object = fetch.body\n}\n",
        )
        .unwrap();
    session.settle(&id).unwrap();
    session
        .publish(&id, apiforge_project::Origin::SESSION)
        .unwrap();
    drop(session);
    project
}

/// Criterion 6: a check in Cleanup that reads a Main step's field marks it
/// too (LAY-20); a check of a component's inner step does not mark the
/// caller's row.
#[test]
fn diag12_criterion_6_cleanup_marks_and_components_do_not() {
    let project = shop_with_checked_component();
    fs::write(
        project.path().join(SCENARIO),
        format!(
            "scenario \"A cart\" {{\n  begin = start\n  thing = checked@1()\n  cart = POST \"/carts\" {{\n{SAMPLE}  }}\n  cleanup {{\n    kept = assert exists cart.body.id\n  }}\n}}\n"
        ),
    )
    .unwrap();
    let (_project, mut harness, id) = opened_in(project, WIDE);
    assert!(
        cleanup_graph(&harness, &id)
            .nodes
            .iter()
            .any(|node| node.handle == "kept"),
        "the check is in Cleanup"
    );
    output_data(&mut harness, &id, "cart");
    assert_eq!(
        mark(&harness, "id").as_deref(),
        Some("exists id"),
        "a Cleanup check marks the Main step's field it reads"
    );
    assert_eq!(mark(&harness, "total"), None);

    output_data(&mut harness, &id, "thing");
    let left = edge(&harness);
    let rows: Vec<String> = harness
        .query_all_by_label(CHECK)
        .filter(|node| node.rect().left() > left)
        .filter_map(|node| node.accesskit_node().description())
        .collect();
    assert!(
        rows.is_empty(),
        "the inner check of the component marks no row of the call: {rows:?}"
    );

    let report = run(&mut harness);
    assert_eq!(report.status, RunStatus::Passed, "{:?}", report.error);
    results_on(&mut harness, "cart");
    let marks = drawn(&mut harness);
    let mark = drawn_on(&marks, true, "body", "id").expect("id is marked in Run results");
    assert_eq!(mark.tone, MarkTone::Passed, "the Cleanup check passed");
    results_on(&mut harness, "thing");
    let marks = drawn(&mut harness);
    assert!(
        marks.iter().all(|mark| !mark.results),
        "the call's rows carry no mark in Run results: {marks:?}"
    );
}

/// Criterion 7: adding or deleting a check updates the marks in the same
/// frame's edit; undo restores them. The index is built once per edit, not
/// once per frame or row.
#[test]
fn diag12_criterion_7_marks_follow_the_edits() {
    let (_project, mut harness, id) = opened(WIDE, &source(CHECKS, ""));
    output_data(&mut harness, &id, "cart");
    assert_eq!(mark(&harness, "id"), None);

    // Drawing builds nothing.
    let builds = CheckIndex::builds();
    for _ in 0..10 {
        harness.step();
    }
    assert_eq!(
        CheckIndex::builds(),
        builds,
        "frames without an edit reuse the index"
    );

    // Adding a check.
    check_on(&harness, "id").click();
    settle(&mut harness);
    in_menu(&harness, "id exists")
        .expect("the menu offers id exists")
        .click();
    harness.remove_cursor();
    settle(&mut harness);
    assert!(
        CheckIndex::builds() > builds,
        "an edit builds the index again"
    );
    output_data(&mut harness, &id, "cart");
    assert_eq!(
        mark(&harness, "id").as_deref(),
        Some("exists id"),
        "the new check marks id"
    );

    // Undo and redo.
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    settle(&mut harness);
    output_data(&mut harness, &id, "cart");
    assert_eq!(mark(&harness, "id"), None, "undo takes the mark away");
    harness.key_press_modifiers(
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        egui::Key::Z,
    );
    settle(&mut harness);
    output_data(&mut harness, &id, "cart");
    assert!(mark(&harness, "id").is_some(), "redo gives it back");

    // Deleting a check.
    let paid = node(&graph(&harness, &id), "paid_true").id.clone();
    app(&mut harness).selections.insert(id.clone(), vec![paid]);
    settle(&mut harness);
    harness.step();
    harness.key_press(egui::Key::Delete);
    settle(&mut harness);
    assert!(
        graph(&harness, &id)
            .nodes
            .iter()
            .all(|node| node.handle != "paid_true"),
        "paid_true is deleted"
    );
    output_data(&mut harness, &id, "cart");
    assert_eq!(
        mark(&harness, "paid"),
        None,
        "deleting the check unmarks paid"
    );
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    settle(&mut harness);
    output_data(&mut harness, &id, "cart");
    assert_eq!(
        mark(&harness, "paid").as_deref(),
        Some(PAID),
        "undo restores it"
    );
}

/// Criterion 8: nothing on the row moves. The marked ✓ stands where the
/// unmarked one does, at both sizes; in Run results the mark is *Check*,
/// right after the value.
#[test]
fn diag12_criterion_8_nothing_on_the_row_moves() {
    for size in [WIDE, MINIMUM] {
        let (_project, mut harness, id) = opened(size, &source(CHECKS, ""));
        output_data(&mut harness, &id, "cart");
        let plain = check_on(&harness, "id").rect();
        for field in ["total", "paid", "owner", "items", "HTTP status"] {
            let marked = check_on(&harness, field).rect();
            assert!(
                (marked.left() - plain.left()).abs() < 0.5
                    && (marked.width() - plain.width()).abs() < 0.5
                    && (marked.height() - plain.height()).abs() < 0.5,
                "the ✓ on {field} stands where id's does at {size:?}: {marked:?} {plain:?}"
            );
        }
    }
}

/// A unit-level count of criterion 7: a response of 1000 fields and 200
/// checks draws its marks over many frames from one index, with no walk of
/// the graph per row.
#[test]
fn diag12_criterion_7_one_index_for_a_large_response() {
    let mut text = String::from(
        "scenario \"Large\" {\n  begin = start\n  big = GET \"/big\" {\n    sample 200 { ",
    );
    for field in 0..1000 {
        text.push_str(&format!("f{field}: {field}, "));
    }
    text.push_str("last: 0 }\n  }\n");
    // A graph holds at most 200 steps: 150 checks in Main, 50 in Cleanup.
    for check in 0..150 {
        text.push_str(&format!(
            "  c{check} = assert big.body.f{check} == {check}\n"
        ));
    }
    text.push_str("  cleanup {\n");
    for check in 150..200 {
        text.push_str(&format!(
            "    c{check} = assert big.body.f{check} == {check}\n"
        ));
    }
    text.push_str("  }\n}\n");
    let (_project, mut harness, id) = opened(WIDE, &text);
    output_data(&mut harness, &id, "big");
    let builds = CheckIndex::builds();
    let marks = drawn(&mut harness);
    assert!(
        marks.iter().filter(|mark| !mark.results).count() >= 50,
        "the rows shown are marked: {}",
        marks.len()
    );
    for _ in 0..20 {
        harness.step();
    }
    assert_eq!(CheckIndex::builds(), builds, "no frame builds the index");
}

/// Design review of DIAG-12 against the board: the Inspector's *Output
/// data* of `cart` with `owner` folded, its ✓ menu on `total`, and Run
/// results of `cart` with the menu of *2 checks*, at 1600 × 1000 and
/// 960 × 540.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_diag12_marks() {
    for (size, suffix) in [(WIDE, "1600"), (MINIMUM, "960")] {
        let (_project, mut harness, id) = opened(size, &source(CHECKS, ""));
        output_data(&mut harness, &id, "cart");
        crate::testing::capture(&mut harness, &format!("diag12-inspector-{suffix}"), None);
        check_on(&harness, "total").click();
        settle(&mut harness);
        crate::testing::capture(
            &mut harness,
            &format!("diag12-inspector-menu-{suffix}"),
            None,
        );
        harness.key_press(egui::Key::Escape);
        settle(&mut harness);

        run(&mut harness);
        results_on(&mut harness, "cart");
        crate::testing::capture(&mut harness, &format!("diag12-results-{suffix}"), None);
        results_check(&harness, "total").click();
        settle(&mut harness);
        crate::testing::capture(&mut harness, &format!("diag12-results-menu-{suffix}"), None);
    }
}

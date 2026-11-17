//! LAY-18 (#302): a reader count names the fields read. Pointing at the
//! count beside a widely read output lists, in the box a chip and a line
//! have (LAY-14), one line per field read, most read first, with the number
//! of its readers and their names (1 = A); a click on the count keeps the
//! list with the port's wires until Esc, a click on the empty board or a
//! second click on the count; the sentence *Draw every wire of this value*
//! goes. After a run each line names the field's last value (3 = A). On
//! `shop-30`, tidied, Compact, its checks folded.
use super::lay14::{LINE, empty_point, line, point_on};
use super::lay16::{press, release};
use super::lay23::{Checks, board, fixture, graph, id_of, revealed, show, tidied, tidied_text};
use super::*;
use crate::testing::MINIMUM;
use egui::{Pos2, Rect, Vec2, vec2};
use serde_json::Value;

const LOGIN_TOKEN: &str = "login.body.token → 10 readers: catalogue, cart, cart_view, address, \
                           checkout, order, +4 more";
const LOGIN_USER: &str = "login.body.userId → 1 check: known_user";
const CART_ID: &str = "cart.body.id → 3 readers: cart_view, address, order";
const CART_TOTAL: &str = "cart.body.total → 2 checks: cart_total, receipt_total";

fn opened(size: Vec2) -> (tempfile::TempDir, FolderHarness, String) {
    let (project, mut harness, id) = tidied("shop-30", size, Checks::Collapsed);
    assert!(board(&harness, &id).debug_compact(), "Compact is the default");
    show(&mut harness, &id, "login", 0.6);
    (project, harness, id)
}

/// Pointing long enough for a tooltip to show.
fn hover(harness: &mut FolderHarness, at: Pos2) {
    harness.hover_at(at);
    for _ in 0..40 {
        harness.step();
    }
}

fn away(harness: &mut FolderHarness) {
    harness.remove_cursor();
    harness.step();
    harness.step();
}

/// A click: the pointer leaves afterwards.
fn click(harness: &mut FolderHarness, at: Pos2) {
    press(harness, at);
    release(harness, at);
    harness.step();
    harness.step();
}

fn escape(harness: &mut FolderHarness) {
    harness.key_press(egui::Key::Escape);
    settle(harness);
    harness.step();
}

/// The count at `handle`'s port `port` as the last frame drew it, and its
/// box on screen.
fn count(
    harness: &FolderHarness,
    id: &str,
    handle: &str,
    port: &str,
) -> (crate::canvas::DrawnCount, Rect) {
    let node = id_of(harness, id, handle);
    let canvas = board(harness, id);
    let drawn = canvas
        .debug_counts()
        .iter()
        .find(|count| count.count.source == node && count.count.port == port)
        .cloned()
        .unwrap_or_else(|| panic!("a count at {handle}.{port}"));
    let rect = canvas.debug_to_screen(drawn.world);
    (drawn, rect)
}

/// The lines of the box whose first line names a field of `prefix`, with
/// the box on screen.
fn list(harness: &FolderHarness, id: &str, prefix: &str) -> Option<(Vec<String>, Rect)> {
    board(harness, id)
        .debug_statements()
        .into_iter()
        .find(|(text, _)| text.starts_with(prefix))
        .map(|(text, rect)| (text.lines().map(str::to_owned).collect(), rect))
}

fn lines(harness: &FolderHarness, id: &str, prefix: &str) -> Vec<String> {
    list(harness, id, prefix)
        .map(|(lines, _)| lines)
        .unwrap_or_else(|| {
            panic!(
                "a list of {prefix}: {:?}",
                board(harness, id).debug_statements()
            )
        })
}

/// No list is shown.
fn assert_no_list(harness: &FolderHarness, id: &str, context: &str) {
    let boxes = board(harness, id).debug_statements();
    assert!(
        boxes.iter().all(|(text, _)| !text.contains(" → 10 readers")),
        "{context}: no list: {boxes:?}"
    );
}

/// Criterion 1: pointing at `login`'s count shows two lines, the field ten
/// steps read first, each with its readers named, at most six; the count
/// keeps its accessible name (criterion 5) and the tooltip sentence is gone.
#[test]
fn lay18_criterion_1_a_count_lists_its_fields() {
    let (_project, mut harness, id) = opened(WIDE);
    let (drawn, rect) = count(&harness, &id, "login", "body");
    assert_eq!(drawn.count.label, "login.body → 11 readers");
    assert_no_list(&harness, &id, "at rest");

    hover(&mut harness, rect.center());
    assert_eq!(
        lines(&harness, &id, "login.body."),
        vec![LOGIN_TOKEN.to_owned(), LOGIN_USER.to_owned()]
    );
    assert_eq!(
        board(&harness, &id).debug_statement().as_deref(),
        Some(format!("{LOGIN_TOKEN}\n{LOGIN_USER}").as_str()),
        "the list is what is pointed at"
    );
    let (_, shown) = list(&harness, &id, "login.body.").unwrap();
    assert!(
        shown.expand(40.0).intersects(rect),
        "the list stands at the count: {shown:?} for {rect:?}"
    );
    assert!(
        harness
            .query_by_label_contains("Draw every wire of this value")
            .is_none(),
        "the list stands in place of the tooltip sentence"
    );
    // Criterion 5: the count's accessible name.
    assert!(
        harness
            .query_by_label("login.body → 11 readers")
            .is_some(),
        "the count keeps its name"
    );

    // Each number is the number of steps that read the field.
    let graph = graph(&harness, &id);
    let login = id_of(&harness, &id, "login");
    let reading = |field: &str| {
        graph
            .nodes
            .iter()
            .filter(|node| {
                node.inputs.values().any(|binding| {
                    binding.references().iter().any(|reference| {
                        reference.node_id == login
                            && reference.port == "body"
                            && reference.path == field
                    })
                })
            })
            .count()
    };
    assert_eq!(reading("token"), 10);
    assert_eq!(reading("userId"), 1);

    away(&mut harness);
    assert_no_list(&harness, &id, "after the pointer leaves");
}

/// Criterion 2: a field nothing reads has no line, and an output without a
/// count is unchanged: its lines are drawn and stated as LAY-14 does.
#[test]
fn lay18_criterion_2_only_fields_read_are_listed() {
    // `login` answers a field nothing reads.
    let text = fixture("shop-30").replace(
        r#"sample 200 { token: "t-1", userId: "u-1" }"#,
        r#"sample 200 { token: "t-1", userId: "u-1", expires: 3600 }"#,
    );
    let (_project, mut harness, id) = tidied_text(&text, WIDE, Checks::Collapsed);
    show(&mut harness, &id, "login", 0.6);
    let (_, rect) = count(&harness, &id, "login", "body");
    hover(&mut harness, rect.center());
    let shown = lines(&harness, &id, "login.body.");
    assert_eq!(
        shown,
        vec![LOGIN_TOKEN.to_owned(), LOGIN_USER.to_owned()],
        "no line for expires"
    );
    away(&mut harness);

    // `valid_cart.value` is read by two steps: no count, its lines drawn.
    show(&mut harness, &id, "checkout", 0.6);
    let valid_cart = id_of(&harness, &id, "valid_cart");
    assert!(
        board(&harness, &id)
            .debug_counts()
            .iter()
            .all(|count| count.count.source != valid_cart),
        "valid_cart has no count"
    );
    let drawn = line(&harness, &id, "valid_cart", "checkout");
    let at = point_on(&harness, &id, &drawn.id);
    hover(&mut harness, at);
    assert_eq!(
        board(&harness, &id).debug_statement().as_deref(),
        Some(LINE),
        "the line states itself as LAY-14 does"
    );
}

/// Criterion 3: a click on the count keeps the list and draws every wire of
/// the port; Esc, a click on the empty board or a second click on the count
/// ends both.
#[test]
fn lay18_criterion_3_a_click_keeps_the_list() {
    let (_project, mut harness, id) = opened(WIDE);
    let (drawn, rect) = count(&harness, &id, "login", "body");
    let all: std::collections::BTreeSet<String> = drawn.count.wires.iter().cloned().collect();
    let kept = |harness: &FolderHarness, context: &str| {
        assert_eq!(
            lines(harness, &id, "login.body."),
            vec![LOGIN_TOKEN.to_owned(), LOGIN_USER.to_owned()],
            "{context}: the list is kept"
        );
        assert!(
            all.is_subset(&revealed(harness, &id)),
            "{context}: every wire of the port is drawn"
        );
    };
    let ended = |harness: &FolderHarness, context: &str| {
        assert_no_list(harness, &id, context);
        assert!(
            revealed(harness, &id).is_disjoint(&all),
            "{context}: the port's wires are chips again"
        );
    };

    click(&mut harness, rect.center());
    away(&mut harness);
    kept(&harness, "clicked");
    escape(&mut harness);
    ended(&harness, "after Esc");

    click(&mut harness, rect.center());
    away(&mut harness);
    kept(&harness, "clicked again");
    let empty = empty_point(&harness, &id);
    click(&mut harness, empty);
    away(&mut harness);
    ended(&harness, "after a click on the empty board");

    click(&mut harness, rect.center());
    away(&mut harness);
    kept(&harness, "clicked a third time");
    click(&mut harness, rect.center());
    away(&mut harness);
    ended(&harness, "after a second click on the count");
}

/// Criterion 4: the list moves no card, no route drawn at rest and no
/// camera, and its box is whole inside the board at 960 × 540 and
/// 1600 × 1000, also with the count at the board's right edge.
#[test]
fn lay18_criterion_4_the_list_moves_nothing_and_fits() {
    for size in [MINIMUM, WIDE] {
        let (_project, mut harness, id) = opened(size);
        let graph = graph(&harness, &id);
        let places = |harness: &FolderHarness| {
            let canvas = board(harness, &id);
            let mut routes: Vec<(String, Vec<Pos2>)> = canvas
                .debug_wires()
                .iter()
                .filter(|wire| !wire.revealed)
                .map(|wire| (wire.id.clone(), wire.points.clone()))
                .collect();
            routes.sort_by(|a, b| a.0.cmp(&b.0));
            (
                canvas.debug_camera(),
                graph
                    .nodes
                    .iter()
                    .map(|node| canvas.debug_world(&node.id))
                    .collect::<Vec<_>>(),
                routes,
            )
        };
        let inside = |harness: &FolderHarness, context: &str| {
            let canvas = board(harness, &id);
            let (_, rect) = list(harness, &id, "login.body.")
                .unwrap_or_else(|| panic!("{size:?} {context}: the list is shown"));
            assert!(
                canvas.debug_board().contains_rect(rect),
                "{size:?} {context}: the list at {rect:?} inside {:?}",
                canvas.debug_board()
            );
        };
        let before = places(&harness);
        let (_, rect) = count(&harness, &id, "login", "body");

        hover(&mut harness, rect.center());
        inside(&harness, "pointing");
        assert_eq!(places(&harness), before, "{size:?}: pointing moves nothing");
        click(&mut harness, rect.center());
        away(&mut harness);
        inside(&harness, "kept");
        let after = places(&harness);
        assert_eq!(
            (after.0, &after.1),
            (before.0, &before.1),
            "{size:?}: a click moves no card and no camera"
        );
        for (wire, points) in &before.2 {
            if let Some((_, now)) = after.2.iter().find(|(other, _)| other == wire) {
                assert_eq!(now, points, "{size:?}: the route of {wire} at rest");
            }
        }
        escape(&mut harness);

        // The count at the board's right edge.
        let edge = board(&harness, &id).debug_board();
        let offset = vec2(edge.right() - 6.0 - rect.right(), 0.0);
        app(&mut harness)
            .canvases
            .get_mut(&id)
            .expect("the board is drawn")
            .debug_pan(offset);
        settle(&mut harness);
        harness.step();
        let (_, moved) = count(&harness, &id, "login", "body");
        hover(&mut harness, moved.center());
        inside(&harness, "at the right edge");
        away(&mut harness);
    }
}

/// The value of `field` of `handle`'s port `port` in the run's report, as
/// the list writes it: JSON, cut to 28 characters.
fn reported(report: &apiforge_core::run::RunResult, handle: &str, port: &str, field: &str) -> String {
    let step = report
        .steps
        .iter()
        .find(|step| step.handle == handle)
        .unwrap_or_else(|| panic!("the report has {handle}"));
    let value: &Value = step
        .outputs
        .get(port)
        .and_then(|value| value.get(field))
        .unwrap_or_else(|| panic!("the report has {handle}.{port}.{field}"));
    let text = value.to_string();
    if text.chars().count() > 28 {
        format!("{}…", text.chars().take(28).collect::<String>())
    } else {
        text
    }
}

fn with_value(line: &str, value: &str) -> String {
    let (field, rest) = line.split_once(" → ").unwrap();
    format!("{field} = {value} → {rest}")
}

/// Criterion 6 (decision 3 = A): after a run each line names its field's
/// value from the report before the arrow, a secret as the report has it,
/// at either density; a field the report has no value for is as without a
/// run.
#[test]
fn lay18_criterion_6_the_list_names_the_run_value() {
    let (_project, mut harness, id) = opened(WIDE);
    choose(&mut harness, "Run simulation");
    let report = finish_run(&mut harness);
    choose(&mut harness, "Diagram");
    settle(&mut harness);
    assert!(board(&harness, &id).debug_compact(), "Compact");

    let token = reported(&report, "login", "body", "token");
    assert_eq!(token, "\"[REDACTED]\"", "the report hides the token");
    let user = reported(&report, "login", "body", "userId");
    show(&mut harness, &id, "login", 0.6);
    let (_, rect) = count(&harness, &id, "login", "body");
    hover(&mut harness, rect.center());
    assert_eq!(
        lines(&harness, &id, "login.body."),
        vec![with_value(LOGIN_TOKEN, &token), with_value(LOGIN_USER, &user)]
    );
    away(&mut harness);

    // At Full, `cart`'s count.
    app(&mut harness).switch_density(crate::canvas::CardDensity::Full);
    settle(&mut harness);
    assert!(!board(&harness, &id).debug_compact(), "Full");
    show(&mut harness, &id, "cart", 1.0);
    let (_, rect) = count(&harness, &id, "cart", "body");
    hover(&mut harness, rect.center());
    let id_value = reported(&report, "cart", "body", "id");
    let total = reported(&report, "cart", "body", "total");
    assert_eq!(id_value, "\"c-1\"");
    assert_eq!(
        lines(&harness, &id, "cart.body."),
        vec![with_value(CART_ID, &id_value), with_value(CART_TOTAL, &total)]
    );
    away(&mut harness);

    // A long value is cut to 28 characters.
    show(&mut harness, &id, "catalogue", 1.0);
    let (_, rect) = count(&harness, &id, "catalogue", "body");
    hover(&mut harness, rect.center());
    let items = reported(&report, "catalogue", "body", "items");
    assert!(items.ends_with('…'), "the items are longer: {items}");
    let shown = lines(&harness, &id, "catalogue.body.");
    assert!(
        shown
            .iter()
            .any(|line| line.starts_with(&format!("catalogue.body.items = {items} → "))),
        "the items' value cut: {shown:?}"
    );
    away(&mut harness);

    // A field the report has no value for: `login` answers no `userId`.
    let text = fixture("shop-30").replace(
        r#"sample 200 { token: "t-1", userId: "u-1" }"#,
        r#"sample 200 { token: "t-1" }"#,
    );
    let (_project, mut harness, id) = tidied_text(&text, WIDE, Checks::Collapsed);
    choose(&mut harness, "Run simulation");
    let report = finish_run(&mut harness);
    choose(&mut harness, "Diagram");
    settle(&mut harness);
    let token = reported(&report, "login", "body", "token");
    show(&mut harness, &id, "login", 0.6);
    let (_, rect) = count(&harness, &id, "login", "body");
    hover(&mut harness, rect.center());
    assert_eq!(
        lines(&harness, &id, "login.body."),
        vec![with_value(LOGIN_TOKEN, &token), LOGIN_USER.to_owned()]
    );
}

//! DIAG-22 (#598) at the window: a step taken from a step's own menu, the one
//! headed "AFTER: <STEP> · What should happen next?" that a right-click on
//! its card or *Add a connected step* at the card's foot opens, is put
//! directly after that step in its braces, unbound, as *Add step* with the
//! step selected and the `+` put it. A check or a value taken from it still
//! reads the step's output. On `rooms/booking-day`, through the gestures a
//! person makes.
use super::diag06a::{graph, id_of, passes, rooms, show, steps};
use super::diag06b::{booking_day, indent, menu_of, position, redone, undone};
use super::diag07::{board, line_of};
use super::lay16::{centre, click_at};
use super::*;
use crate::testing::MINIMUM;
use apiforge_core::format::{Graph, Node};
use egui::Rect;
use std::collections::HashMap;

const SCENARIO: &str = "scenarios/booking-day.afs";

/// The open *Add step* menu on screen.
fn popup(harness: &FolderHarness) -> Rect {
    harness
        .ctx
        .memory(|memory| memory.area_rect(egui::Id::new("board-action-menu")))
        .expect("the menu is drawn")
}

/// Types `label` into the open menu's *Find an action…* and clicks the entry
/// of that name; returns the block it added.
fn take(harness: &mut FolderHarness, id: &str, label: &str) -> Node {
    assert!(board(harness, id).debug_menu_open(), "the menu is open");
    let before = graph(harness, id);
    harness.event(egui::Event::Text(label.to_owned()));
    harness.step();
    harness.step();
    let menu = popup(harness);
    let entry = harness
        .query_all_by_label(label)
        .map(|node| node.rect())
        .filter(|rect| menu.contains_rect(*rect))
        .min_by(|a, b| a.top().total_cmp(&b.top()))
        .unwrap_or_else(|| panic!("the menu offers {label}"));
    click_at(harness, entry.center(), egui::PointerButton::Primary, 1);
    settle(harness);
    harness.step();
    let after = graph(harness, id);
    assert_eq!(after.nodes.len(), before.nodes.len() + 1, "one block added");
    after
        .nodes
        .iter()
        .find(|node| !before.nodes.iter().any(|old| old.id == node.id))
        .expect("the new block")
        .clone()
}

/// Right-clicks `handle`, whose menu keeps its heading, and takes `label`.
fn from_menu(harness: &mut FolderHarness, id: &str, handle: &str, label: &str) -> Node {
    menu_of(harness, id, handle);
    headed(harness, handle);
    take(harness, id, label)
}

fn headed(harness: &FolderHarness, handle: &str) {
    let heading = format!("AFTER: {}", handle.to_uppercase());
    assert!(
        harness.query_by_label(&heading).is_some(),
        "the menu is headed {heading}"
    );
    assert!(harness.query_by_label("What should happen next?").is_some());
}

/// Selects `handles` and opens *Add step* on the board's bar.
fn bar(harness: &mut FolderHarness, id: &str, handles: &[&str]) {
    let model = graph(harness, id);
    let ids = handles.iter().map(|handle| id_of(&model, handle)).collect();
    app(harness).selections.insert(id.to_owned(), ids);
    settle(harness);
    harness.step();
    harness.get_by_label("Add step").click();
    settle(harness);
    harness.step();
}

/// Turns *Compact cards* off and tidies the board.
fn full(harness: &mut FolderHarness) {
    app(harness).switch_density(crate::canvas::CardDensity::Full);
    settle(harness);
    choose(harness, "Tidy up");
    settle(harness);
}

/// Opens the menu of *Add a connected step* at the foot of `handle`'s card,
/// which only the full card has.
fn foot(harness: &mut FolderHarness, id: &str, handle: &str) {
    full(harness);
    show(harness, id, &[handle]);
    let button = centre(harness, &format!("Add a step connected to {handle}"));
    click_at(harness, button, egui::PointerButton::Primary, 1);
    harness.step();
    headed(harness, handle);
}

/// `new` runs directly after `step` and `next` directly after `new`, its text
/// stands between theirs at their indentation, it reads nothing of `step`,
/// the edit passes the check, and one Undo gives `before` back.
fn put_after(
    harness: &mut FolderHarness,
    id: &str,
    (step, new, next): (&str, &Node, Option<&str>),
    before: &str,
) -> String {
    let text = passes(harness, id);
    let order = steps(harness, id);
    assert_eq!(
        position(&order, &new.handle),
        position(&order, step) + 1,
        "`{}` runs directly after `{step}`: {order:?}",
        new.handle
    );
    let written = line_of(&text, &format!("{} = ", new.handle));
    assert!(line_of(&text, &format!("{step} = ")) < written, "{text}");
    if let Some(next) = next {
        assert_eq!(
            position(&order, next),
            position(&order, &new.handle) + 1,
            "`{next}` runs directly after `{}`: {order:?}",
            new.handle
        );
        assert!(written < line_of(&text, &format!("{next} = ")), "{text}");
    }
    assert_eq!(
        indent(&text, &new.handle),
        indent(&text, step),
        "in the braces of `{step}`: {text}"
    );
    let source = id_of(&graph(harness, id), step);
    let reads = serde_json::to_string(&new.inputs).unwrap();
    assert!(!reads.contains(&source), "it reads `{step}`: {reads}");
    assert!(
        !statement(&text, &new.handle).contains(step),
        "it names `{step}`: {text}"
    );
    undone(harness, id, before);
    redone(harness, id, &text);
    text
}

/// The text of the statement `handle`, to its closing brace when it has one.
fn statement(text: &str, handle: &str) -> String {
    let from = line_of(text, &format!("{handle} = "));
    let lines: Vec<&str> = text.lines().skip(from).collect();
    let close = format!("{}}}", " ".repeat(indent(text, handle)));
    let end = match lines[0].ends_with('{') {
        true => lines.iter().position(|line| *line == close).unwrap() + 1,
        false => 1,
    };
    lines[..end].join("\n")
}

fn places(graph: &Graph) -> HashMap<String, (f64, f64)> {
    graph
        .nodes
        .iter()
        .map(|node| {
            let at = &node.position;
            (node.handle.clone(), (at.x, at.y))
        })
        .collect()
}

/// The board is what *Add step* with `profile` selected gives for `label` on
/// a fresh copy at the same density: the same order, the same text and every
/// card at the same place (criterion 6, as amended on #598).
fn same_as_add_step(harness: &FolderHarness, id: &str, label: &str) {
    let (_project, mut reference, other) = booking_day();
    if !board(harness, id).debug_compact() {
        full(&mut reference);
    }
    bar(&mut reference, &other, &["profile"]);
    take(&mut reference, &other, label);
    assert_eq!(
        board(harness, id).debug_compact(),
        board(&reference, &other).debug_compact()
    );
    assert_eq!(steps(harness, id), steps(&reference, &other), "{label}");
    assert_eq!(
        source_of(harness, id),
        source_of(&reference, &other),
        "{label}"
    );
    assert_eq!(
        places(&graph(harness, id)),
        places(&graph(&reference, &other)),
        "{label}: room is made alike"
    );
}

// Criterion 1 ----------------------------------------------------------------

/// The reproduction of AB-02's wrong turn in run 2 (#583).
#[test]
fn diag22_criterion_1_a_request_from_a_steps_menu() {
    for size in [MINIMUM, WIDE] {
        let (_project, mut harness, id) = booking_day();
        harness.set_size(size);
        settle(&mut harness);
        let before = source_of(&harness, &id);
        let new = from_menu(&mut harness, &id, "profile", "HTTP request");
        let text = put_after(&mut harness, &id, ("profile", &new, Some("team")), &before);
        assert!(!new.inputs.contains_key("body"), "no body: {new:?}");
        assert!(!statement(&text, &new.handle).contains("{{"), "{text}");
    }
}

#[test]
fn diag22_criterion_1_same_as_add_step_with_the_step_selected() {
    let (_project, mut harness, id) = booking_day();
    from_menu(&mut harness, &id, "profile", "HTTP request");
    same_as_add_step(&harness, &id, "HTTP request");
}

// Criterion 2 ----------------------------------------------------------------

#[test]
fn diag22_criterion_2_from_the_cards_button() {
    for label in ["HTTP request", "Sign in", "IF / ELSE"] {
        let (_project, mut harness, id) = booking_day();
        foot(&mut harness, &id, "profile");
        let before = source_of(&harness, &id);
        let new = take(&mut harness, &id, label);
        same_as_add_step(&harness, &id, label);
        put_after(&mut harness, &id, ("profile", &new, Some("team")), &before);
    }
}

/// A component call and an *IF / ELSE* from the right-click menu.
#[test]
fn diag22_criterion_2_a_component_and_an_if_from_a_steps_menu() {
    for label in ["Sign in", "IF / ELSE"] {
        let (_project, mut harness, id) = booking_day();
        let before = source_of(&harness, &id);
        let new = from_menu(&mut harness, &id, "profile", label);
        same_as_add_step(&harness, &id, label);
        put_after(&mut harness, &id, ("profile", &new, Some("team")), &before);
    }
}

#[test]
fn diag22_criterion_2_in_braces() {
    for (step, next) in [("second", Some("calendar")), ("calendar", None)] {
        for label in ["HTTP request", "Sign in", "IF / ELSE"] {
            let (_project, mut harness, id) = booking_day();
            let before = source_of(&harness, &id);
            let new = from_menu(&mut harness, &id, step, label);
            let text = put_after(&mut harness, &id, (step, &new, next), &before);
            assert_eq!(indent(&text, &new.handle), 4, "{text}");
            let written = line_of(&text, &format!("{} = ", new.handle));
            assert!(
                line_of(&text, "booked = if") < written && written < line_of(&text, "} else {"),
                "in the *Yes* of `booked`: {text}"
            );
        }
    }
    // From the button at the card's foot.
    let (_project, mut harness, id) = booking_day();
    foot(&mut harness, &id, "calendar");
    let before = source_of(&harness, &id);
    let new = take(&mut harness, &id, "HTTP request");
    put_after(&mut harness, &id, ("calendar", &new, None), &before);
}

// Criterion 3 ----------------------------------------------------------------

/// A second component, beside `sign_in`, which takes no data of `profile`.
const SIGN_OUT: &str = "component sign_out \"Sign out\" {
  description \"Close the session.\"
  closed = DELETE \"/sessions\" {
    sample 204 {}
  }
}
";

const COMPONENTS: [&str; 2] = ["Sign in", "Sign out"];

/// The SHA-256 of [`SIGN_OUT`].
const SIGN_OUT_HASH: &str = "9d3786773c94c56ec2674b9549970d70e5e44cc13285d35c6d6a9f4db5d727df";

fn with_sign_out() -> (tempfile::TempDir, FolderHarness, String) {
    rooms(SCENARIO, |root| {
        let revisions = root.join(".apiforge/revisions/sign-out");
        fs::create_dir_all(&revisions).unwrap();
        fs::write(revisions.join("1.afs"), SIGN_OUT).unwrap();
        fs::write(root.join("components/sign-out.afs"), SIGN_OUT).unwrap();
        let path = root.join("apiforge.project.json");
        let mut manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        manifest["documents"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "documentId": "6f1d2c40-1a7e-4c55-9b1e-2a1f0c9d7e05",
                "kind": "component",
                "path": "components/sign-out.afs",
                "entityId": "sign-out",
                "baseRevision": 1,
                "recovered": false
            }));
        manifest["revisions"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "componentId": "sign-out",
                "handle": "sign_out",
                "revision": 1,
                "path": ".apiforge/revisions/sign-out/1.afs",
                "sourceHash": SIGN_OUT_HASH
            }));
        fs::write(&path, serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
    })
}

/// Which of `COMPONENTS` the open menu lists when *Sign* is searched for,
/// those its scrolling list holds below the view among them.
fn offered(harness: &mut FolderHarness) -> Vec<&'static str> {
    harness.event(egui::Event::Text("Sign".to_owned()));
    harness.step();
    harness.step();
    let menu = popup(harness);
    let found = COMPONENTS
        .into_iter()
        .filter(|name| {
            harness.query_all_by_label(name).any(|node| {
                let rect = node.rect();
                menu.x_range().contains(rect.left())
                    && menu.x_range().contains(rect.right())
                    && rect.top() > menu.top()
            })
        })
        .collect();
    harness.key_press(egui::Key::Escape);
    settle(harness);
    found
}

#[test]
fn diag22_criterion_3_every_component_is_offered() {
    let (_project, mut harness, id) = with_sign_out();
    bar(&mut harness, &id, &["profile"]);
    assert_eq!(offered(&mut harness), COMPONENTS, "*Add step* on the bar");
    menu_of(&mut harness, &id, "profile");
    assert_eq!(offered(&mut harness), COMPONENTS, "the menu of `profile`");
    let (_project, mut harness, id) = with_sign_out();
    foot(&mut harness, &id, "profile");
    assert_eq!(offered(&mut harness), COMPONENTS, "the button of `profile`");
}

// Criterion 4 ----------------------------------------------------------------

/// `text` with `statement` written before the line `line`.
fn written_before(text: &str, line: &str, statement: &str) -> String {
    assert_eq!(text.matches(line).count(), 1, "one line {line}");
    text.replace(line, &format!("{statement}{line}"))
}

/// Pinned to what `main` wrote before this ticket.
#[test]
fn diag22_criterion_4_a_check_and_a_value_keep_their_binding() {
    for (label, statement) in [
        ("Comparison", "  check = assert profile.body == null\n"),
        ("Value", "  value = profile.body\n"),
        ("Object", "  object = { field: profile.body }\n"),
        ("Variable", "  var variable = profile.body\n"),
    ] {
        let (_project, mut harness, id) = booking_day();
        let before = source_of(&harness, &id);
        from_menu(&mut harness, &id, "profile", label);
        assert_eq!(
            passes(&harness, &id),
            written_before(&before, "  cleanup {\n", statement),
            "{label}"
        );
    }
}

// Criterion 5 ----------------------------------------------------------------

/// Pinned to what `main` wrote before this ticket.
#[test]
fn diag22_criterion_5_start_and_if_unchanged() {
    let request = "  request = GET \"/endpoint\" {\n    timeout 10s\n  }\n";
    let nested = "    request = GET \"/endpoint\" {\n      timeout 10s\n    }\n";
    for (handle, line, statement) in [
        ("begin", "  token = sign_in@1(user: \"tester\")\n", request),
        ("booked", "  } else {\n", nested),
    ] {
        let (_project, mut harness, id) = booking_day();
        let before = source_of(&harness, &id);
        from_menu(&mut harness, &id, handle, "HTTP request");
        assert_eq!(
            passes(&harness, &id),
            written_before(&before, line, statement),
            "{handle}"
        );
    }
}

/// A check is no step: a request from its menu reads it, as before.
#[test]
fn diag22_criterion_5_a_checks_menu_unchanged() {
    let (_project, mut harness, id) = booking_day();
    app(&mut harness).debug_draw_checks();
    settle(&mut harness);
    choose(&mut harness, "Tidy up");
    settle(&mut harness);
    let before = source_of(&harness, &id);
    from_menu(&mut harness, &id, "big_enough", "HTTP request");
    assert_eq!(
        passes(&harness, &id),
        written_before(
            &before,
            "  cleanup {\n",
            "  @config({ method: \"GET\", timeoutMs: 10000.0 })\n  request = GET \"/endpoint\" {\n    \
             body big_enough.value\n    timeout 10s\n  }\n"
        )
    );
}

// Design review -------------------------------------------------------------

/// The menu of `profile`, and the board after an *HTTP request* is taken
/// from it, at 960 × 540 and 1600 × 1000, on the board as it opens.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_diag22() {
    for (size, suffix) in [(MINIMUM, "960"), (WIDE, "1600")] {
        let (project, closed, _) = booking_day();
        drop(closed);
        let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
        app(&mut harness).review_open(SCENARIO, None);
        settle(&mut harness);
        harness.step();
        let id = folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .documents()
            .iter()
            .find(|document| document.entry().path.as_str() == SCENARIO)
            .unwrap()
            .id()
            .to_owned();
        menu_of(&mut harness, &id, "profile");
        crate::testing::capture(&mut harness, &format!("diag22-menu-{suffix}"), None);
        take(&mut harness, &id, "HTTP request");
        crate::testing::capture(&mut harness, &format!("diag22-added-{suffix}"), None);
        choose(&mut harness, "Fit the view");
        settle(&mut harness);
        crate::testing::capture(&mut harness, &format!("diag22-fit-{suffix}"), None);
    }
}

//! DIAG-06b (#491) at the window: a wire carried over a step that will
//! refuse it crosses the socket, turns red and says why at the pointer, and
//! the banner keeps the sentence on release; a deleted step in the middle
//! of a chain leaves a notice that names what now follows what, with
//! *Undo*; pointing at a line of a chain shows a `+` that adds a step on
//! that line; *Move earlier* and *Move later* in a step's menu, and
//! Alt+↑ and Alt+↓ on the board, move the selected step by one step in its
//! braces. Every edit passes the check and is one step of Undo. On
//! `rooms/booking-day`, its `sign_in` component and the demo shop, through
//! the gestures a person makes.
use super::diag06a::{graph, id_of, passes, rooms, shop_with, show, steps};
use super::diag07::{board, empty_point, line_of};
use super::lay16::{centre, click_at, move_to, press, release};
use super::*;
use crate::testing::MINIMUM;
use egui::{Pos2, Rect};
use egui_kittest::kittest::NodeT;

const SCENARIO: &str = "scenarios/booking-day.afs";
const OWN_BRACES: &str = "A condition cannot go into its own braces.";
const ONLY_STEPS: &str = "Only steps are ordered. A check runs when what it reads exists.";
const MOVE_EARLIER: &str = "Move earlier";
const MOVE_LATER: &str = "Move later";

/// The demo shop's scenario where `pay` reads `cart` (DIAG-06a).
const PAY: &str = r#"scenario "Pay" {
  begin = start
  cart = POST "/cart" {
    sample 201 { id: "c-1", total: 10 }
  }
  cart_id = cart.body.id
  pay = POST "/pay/{{cart.body.id}}" {
    sample 200 { ok: true }
  }
  done = GET "/done" {
    sample 200 {}
  }
  receipt = GET "/receipts/{{cart_id}}" {
    sample 200 {}
  }
}
"#;

/// The room project's component with three steps that read nothing of one
/// another.
const SIGN_IN: &str = "component sign_in \"Sign in\" {
  description \"Open a session on the room booking API and return its token.\"
  @label(\"User\")
  input user: string = \"tester\"
  session = POST \"/sessions\" {
    body { user: param.user, password: \"secret\" }
    sample 201 { token: \"tok-1\" }
  }
  health = GET \"/health\" {
    sample 200 { ok: true }
  }
  audit = GET \"/audit\" {
    sample 200 { ok: true }
  }
  export token: string = session.body.token
}
";

fn booking_day() -> (tempfile::TempDir, FolderHarness, String) {
    rooms(SCENARIO, |_| {})
}

/// `booking-day` with its checks drawn as cards, so a wire can be carried
/// over one.
fn with_checks() -> (tempfile::TempDir, FolderHarness, String) {
    let (project, mut harness, id) = booking_day();
    app(&mut harness).debug_draw_checks();
    settle(&mut harness);
    choose(&mut harness, "Tidy up");
    settle(&mut harness);
    harness.step();
    (project, harness, id)
}

fn sign_in() -> (tempfile::TempDir, FolderHarness, String) {
    rooms("components/sign-in.afs", |root| {
        fs::write(root.join("components/sign-in.afs"), SIGN_IN).unwrap();
    })
}

fn screen(harness: &FolderHarness, id: &str, world: Pos2) -> Pos2 {
    board(harness, id)
        .debug_to_screen(Rect::from_min_max(world, world))
        .min
}

/// One Undo gives the file back byte for byte.
fn undone(harness: &mut FolderHarness, id: &str, before: &str) {
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    settle(harness);
    harness.step();
    assert_eq!(
        source_of(harness, id),
        before,
        "one Undo gives the file back"
    );
}

/// Redo gives the edit back.
fn redone(harness: &mut FolderHarness, id: &str, after: &str) {
    harness.key_press_modifiers(
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        egui::Key::Z,
    );
    settle(harness);
    harness.step();
    assert_eq!(source_of(harness, id), after, "Redo gives the edit back");
}

fn position(order: &[String], handle: &str) -> usize {
    super::diag06a::at(order, handle)
}

fn indent(text: &str, handle: &str) -> usize {
    let line = text
        .lines()
        .nth(line_of(text, &format!("{handle} = ")))
        .unwrap();
    line.len() - line.trim_start().len()
}

// Criterion 1 ----------------------------------------------------------------

/// Carries a wire from the socket `port` of `source` over the order socket
/// of `target` and holds it there: the socket is crossed and the line red
/// (the board's refusal names the target), and `reason` stands at the
/// pointer and in the accessibility tree. Released, nothing changes and the
/// banner holds the same sentence.
fn refused_while_carried(
    harness: &mut FolderHarness,
    id: &str,
    (source, port, target): (&str, &str, &str),
    reason: &str,
) {
    show(harness, id, &[source, target]);
    let before = source_of(harness, id);
    let model = graph(harness, id);
    let target_id = id_of(&model, target);
    let from = centre(harness, &format!("Output {port} of {source}"));
    let (socket, _) = board(harness, id).debug_order_sockets(&target_id);
    let to = screen(harness, id, socket.expect("the target has an order socket"));
    press(harness, from);
    move_to(harness, from, to);
    harness.step();

    assert_eq!(
        board(harness, id).debug_refusal(),
        Some((target_id.as_str(), reason)),
        "while carried over `{target}`: the socket crossed, the line red"
    );
    assert_eq!(
        board(harness, id).debug_caption(),
        Some(reason),
        "the reason at the pointer"
    );
    assert!(
        harness.query_by_label(reason).is_some(),
        "the reason in the accessibility tree"
    );

    release(harness, to);
    harness.step();
    assert_eq!(source_of(harness, id), before, "the source is unchanged");
    assert_eq!(graph(harness, id), model, "the model is unchanged");
    assert_eq!(board(harness, id).debug_refusal(), None, "released");
    assert_eq!(board(harness, id).debug_caption(), None, "released");
    assert!(
        harness.query_by_label(reason).is_some(),
        "the banner holds the same sentence: {reason}"
    );
}

#[test]
fn diag06b_criterion_1_a_value_read() {
    let (_project, mut harness, id) = shop_with(PAY);
    refused_while_carried(
        &mut harness,
        &id,
        ("begin", "__order", "pay"),
        "`pay` reads `cart.body.id`; it stays below `cart`.",
    );
}

#[test]
fn diag06b_criterion_1_a_check_from_next() {
    let (_project, mut harness, id) = with_checks();
    refused_while_carried(&mut harness, &id, ("first", "__order", "sent"), ONLY_STEPS);
}

#[test]
fn diag06b_criterion_1_an_if_into_itself() {
    let (_project, mut harness, id) = with_checks();
    refused_while_carried(
        &mut harness,
        &id,
        ("second", "__order", "booked"),
        OWN_BRACES,
    );
}

/// A wire the step takes shows no refusal: the caption of DIAG-06c.
#[test]
fn diag06b_criterion_1_a_wire_that_is_taken_shows_none() {
    let (_project, mut harness, id) = booking_day();
    show(&mut harness, &id, &["first", "invite"]);
    let target = id_of(&graph(&harness, &id), "invite");
    let from = centre(&harness, "Output __order of first");
    let (socket, _) = board(&harness, &id).debug_order_sockets(&target);
    let to = screen(&harness, &id, socket.expect("an order socket"));
    press(&mut harness, from);
    move_to(&mut harness, from, to);
    harness.step();
    assert_eq!(board(&harness, &id).debug_refusal(), None);
    assert_eq!(
        board(&harness, &id).debug_caption(),
        Some("Move `invite` after `first`")
    );
    release(&mut harness, to);
}

// Criterion 2 ----------------------------------------------------------------

/// Selects the statements `handles` and presses Delete.
fn delete(harness: &mut FolderHarness, id: &str, handles: &[&str]) {
    let board_graph = graph(harness, id);
    let ids = handles
        .iter()
        .map(|handle| id_of(&board_graph, handle))
        .collect();
    app(harness).selections.insert(id.to_owned(), ids);
    settle(harness);
    harness.step();
    harness.key_press(egui::Key::Delete);
    settle(harness);
    harness.step();
}

fn notice(harness: &FolderHarness) -> Option<String> {
    harness
        .query_by_label_contains("now follows")
        .and_then(|node| node.accesskit_node().label())
}

const POLICY_GONE: &str = "`policy` deleted. `holidays` now follows `equipment`.";

#[test]
fn diag06b_criterion_2_a_middle_step() {
    let (_project, mut harness, id) = booking_day();
    let before = source_of(&harness, &id);
    delete(&mut harness, &id, &["policy"]);
    let after = passes(&harness, &id);
    assert_ne!(after, before, "the step is deleted");
    assert_eq!(notice(&harness).as_deref(), Some(POLICY_GONE));
    // A few frames with nothing done: it stays.
    for _ in 0..10 {
        harness.step();
    }
    assert_eq!(notice(&harness).as_deref(), Some(POLICY_GONE), "it stays");
    let order = steps(&harness, &id);
    assert_eq!(
        position(&order, "holidays"),
        position(&order, "equipment") + 1,
        "{order:?}"
    );
    // *Undo* restores the step, as one step of Undo.
    let undo = centre(&harness, "Undo");
    click_at(&mut harness, undo, egui::PointerButton::Primary, 1);
    settle(&mut harness);
    harness.step();
    assert_eq!(source_of(&harness, &id), before, "Undo restores the step");
    assert_eq!(notice(&harness), None, "the notice goes with Undo");
    redone(&mut harness, &id, &after);
}

#[test]
fn diag06b_criterion_2_the_notice_goes_with_the_next_action() {
    let (_project, mut harness, id) = booking_day();
    delete(&mut harness, &id, &["policy"]);
    assert_eq!(notice(&harness).as_deref(), Some(POLICY_GONE));
    let text = source_of(&harness, &id);
    let at = empty_point(&harness, &id);
    click_at(&mut harness, at, egui::PointerButton::Primary, 1);
    settle(&mut harness);
    harness.step();
    assert_eq!(notice(&harness), None, "a click on the board is an action");
    assert_eq!(source_of(&harness, &id), text, "and changes nothing");

    let (_project, mut harness, id) = booking_day();
    delete(&mut harness, &id, &["policy"]);
    assert_eq!(notice(&harness).as_deref(), Some(POLICY_GONE));
    harness.key_press(egui::Key::Escape);
    settle(&mut harness);
    harness.step();
    assert_eq!(notice(&harness), None, "a key is an action");
}

#[test]
fn diag06b_criterion_2_the_last_step_or_a_check_shows_none() {
    let (_project, mut harness, id) = sign_in();
    let before = source_of(&harness, &id);
    delete(&mut harness, &id, &["audit"]);
    passes(&harness, &id);
    assert_ne!(source_of(&harness, &id), before, "the last step is deleted");
    assert_eq!(notice(&harness), None, "the last step");
    undone(&mut harness, &id, &before);

    let (_project, mut harness, id) = booking_day();
    let before = source_of(&harness, &id);
    app(&mut harness).debug_draw_checks();
    settle(&mut harness);
    delete(&mut harness, &id, &["has_screen"]);
    passes(&harness, &id);
    assert_ne!(source_of(&harness, &id), before, "the check is deleted");
    assert_eq!(notice(&harness), None, "a check");
    undone(&mut harness, &id, &before);
}

// Criterion 3 ----------------------------------------------------------------

fn plus(source: &str, target: &str) -> String {
    format!("Add a step between `{source}` and `{target}`")
}

/// The line of the chain from `source` to `target` the last frame drew.
fn chain_line(harness: &FolderHarness, id: &str, source: &str, target: &str) -> String {
    let board_graph = graph(harness, id);
    let (source, target) = (id_of(&board_graph, source), id_of(&board_graph, target));
    board(harness, id)
        .debug_wires()
        .iter()
        .find(|wire| {
            wire.chain && wire.source.as_deref() == Some(source.as_str()) && wire.target == target
        })
        .expect("the line is drawn")
        .id
        .clone()
}

/// Points at the line from `source` to `target`, activates its `+` and
/// adds an HTTP request from the menu that opens; returns the new step.
fn add_on(harness: &mut FolderHarness, id: &str, source: &str, target: &str) -> String {
    let label = plus(source, target);
    assert!(
        harness.query_by_label(&label).is_none(),
        "no `+` before the line is pointed at"
    );
    let line = chain_line(harness, id, source, target);
    let at = super::lay14::point_on(harness, id, &line);
    harness.hover_at(at);
    harness.step();
    harness.step();
    let rect = harness.get_by_label(&label).rect();
    crate::testing::assert_within_window(harness, &[&label]);
    assert!(
        rect.width() >= 16.0 && rect.height() >= 16.0,
        "the `+` is at least 16 px: {rect:?}"
    );
    let before = graph(harness, id);
    click_at(harness, rect.center(), egui::PointerButton::Primary, 1);
    harness.step();
    assert!(board(harness, id).debug_menu_open(), "*Add step* opens");
    harness
        .get_all_by_label("HTTP request")
        .min_by(|a, b| {
            let distance = |rect: Rect| rect.center().distance(at);
            distance(a.rect()).total_cmp(&distance(b.rect()))
        })
        .expect("the menu offers a request")
        .click();
    settle(harness);
    harness.step();
    let after = graph(harness, id);
    assert_eq!(after.nodes.len(), before.nodes.len() + 1, "one step added");
    after
        .nodes
        .iter()
        .find(|node| !before.nodes.iter().any(|old| old.id == node.id))
        .expect("the new step")
        .handle
        .clone()
}

fn diag06b_criterion_3_on_a_line_at(size: egui::Vec2) {
    let (_project, mut harness, id) = booking_day();
    harness.set_size(size);
    settle(&mut harness);
    show(&mut harness, &id, &["first", "hold"]);
    let before = source_of(&harness, &id);
    let new = add_on(&mut harness, &id, "first", "hold");
    let text = passes(&harness, &id);
    let order = steps(&harness, &id);
    assert_eq!(
        position(&order, &new),
        position(&order, "first") + 1,
        "{size:?}: {order:?}"
    );
    assert_eq!(
        position(&order, "hold"),
        position(&order, &new) + 1,
        "{size:?}: {order:?}"
    );
    assert!(
        line_of(&text, "first = ") < line_of(&text, &format!("{new} = "))
            && line_of(&text, &format!("{new} = ")) < line_of(&text, "hold = "),
        "{size:?}: {text}"
    );
    undone(&mut harness, &id, &before);
}

#[test]
fn diag06b_criterion_3_a_plus_on_a_line() {
    diag06b_criterion_3_on_a_line_at(MINIMUM);
    diag06b_criterion_3_on_a_line_at(WIDE);
}

#[test]
fn diag06b_criterion_3_a_plus_in_braces() {
    let (_project, mut harness, id) = booking_day();
    show(&mut harness, &id, &["second", "calendar"]);
    let before = source_of(&harness, &id);
    let new = add_on(&mut harness, &id, "second", "calendar");
    let text = passes(&harness, &id);
    let order = steps(&harness, &id);
    assert_eq!(
        position(&order, &new),
        position(&order, "second") + 1,
        "{order:?}"
    );
    assert_eq!(indent(&text, &new), 4, "in the braces of `booked`: {text}");
    assert!(
        line_of(&text, &format!("{new} = ")) < line_of(&text, "calendar = "),
        "{text}"
    );
    undone(&mut harness, &id, &before);
}

// Criterion 4 ----------------------------------------------------------------

/// Opens the menu of the card `handle` with the secondary button.
fn menu_of(harness: &mut FolderHarness, id: &str, handle: &str) {
    show(harness, id, &[handle]);
    let card = board(harness, id)
        .debug_card_rect(&id_of(&graph(harness, id), handle))
        .unwrap_or_else(|| panic!("`{handle}` is drawn"));
    click_at(harness, card.center(), egui::PointerButton::Secondary, 1);
    harness.step();
    assert!(
        board(harness, id).debug_menu_open(),
        "the step's menu opens"
    );
}

fn disabled(harness: &FolderHarness, label: &str) -> bool {
    harness.get_by_label(label).accesskit_node().is_disabled()
}

/// Points at the menu entry `label` long enough for its hover text, which
/// must be `reason`.
fn says_why(harness: &mut FolderHarness, label: &str, reason: &str) {
    assert!(disabled(harness, label), "{label} is disabled");
    let at = centre(harness, label);
    harness.hover_at(at);
    for _ in 0..40 {
        harness.step();
    }
    assert!(
        harness.query_by_label(reason).is_some(),
        "{label}'s hover text: {reason}"
    );
}

/// Takes `label` from the menu of `handle`: the source changes, passes the
/// check, and one Undo gives it back; Redo leaves the move in place.
fn moved_by_menu(harness: &mut FolderHarness, id: &str, handle: &str, label: &str) -> Vec<String> {
    let before = source_of(harness, id);
    menu_of(harness, id, handle);
    assert!(
        !disabled(harness, label),
        "{label} is enabled on `{handle}`"
    );
    let entry = centre(harness, label);
    click_at(harness, entry, egui::PointerButton::Primary, 1);
    settle(harness);
    harness.step();
    let after = passes(harness, id);
    assert_ne!(after, before, "{label} moves `{handle}`");
    let order = steps(harness, id);
    undone(harness, id, &before);
    redone(harness, id, &after);
    order
}

#[test]
fn diag06b_criterion_4_move_later_and_earlier_from_the_menu() {
    let (_project, mut harness, id) = booking_day();
    let original = steps(&harness, &id);
    let order = moved_by_menu(&mut harness, &id, "hold", MOVE_LATER);
    assert_eq!(
        order[position(&order, "first")..position(&order, "booked")],
        ["first", "invite", "hold", "notice"]
    );
    let text = source_of(&harness, &id);
    // With its check, which reads only it.
    assert!(line_of(&text, "invited = ") < line_of(&text, "hold = "));
    assert_eq!(
        line_of(&text, "held = "),
        line_of(&text, "hold = ") + 5,
        "{text}"
    );
    let order = moved_by_menu(&mut harness, &id, "hold", MOVE_EARLIER);
    assert_eq!(order, original);
}

#[test]
fn diag06b_criterion_4_in_braces_and_with_an_if() {
    let (_project, mut harness, id) = booking_day();
    let order = moved_by_menu(&mut harness, &id, "calendar", MOVE_EARLIER);
    assert_eq!(
        order[position(&order, "booked")..],
        ["booked", "calendar", "second"]
    );
    let text = source_of(&harness, &id);
    assert_eq!(indent(&text, "calendar"), 4, "{text}");
    assert_eq!(indent(&text, "second"), 4, "{text}");
    // An `if` moves with its braces, and a step moves past a whole `if`.
    let order = moved_by_menu(&mut harness, &id, "booked", MOVE_EARLIER);
    assert_eq!(
        order[position(&order, "invite")..],
        ["invite", "booked", "calendar", "second", "notice"]
    );
}

#[test]
fn diag06b_criterion_4_disabled_with_the_reason() {
    let (_project, mut harness, id) = booking_day();
    menu_of(&mut harness, &id, "second");
    says_why(
        &mut harness,
        MOVE_EARLIER,
        "`second` is already the first step in its braces.",
    );
    harness.key_press(egui::Key::Escape);
    settle(&mut harness);

    menu_of(&mut harness, &id, "calendar");
    says_why(
        &mut harness,
        MOVE_LATER,
        "`calendar` is already the last step in its braces.",
    );
    harness.key_press(egui::Key::Escape);
    settle(&mut harness);

    menu_of(&mut harness, &id, "booked");
    says_why(
        &mut harness,
        MOVE_LATER,
        "`booked` is already the last step in its braces.",
    );
    harness.key_press(egui::Key::Escape);
    settle(&mut harness);

    // `hold` reads `first`.
    menu_of(&mut harness, &id, "hold");
    says_why(
        &mut harness,
        MOVE_EARLIER,
        "`hold` reads `first.body.id`; it stays below `first`.",
    );
}

fn alt(harness: &mut FolderHarness, key: egui::Key) {
    harness.key_press_modifiers(egui::Modifiers::ALT, key);
    settle(harness);
    harness.step();
}

#[test]
fn diag06b_criterion_4_the_keys() {
    let (_project, mut harness, id) = booking_day();
    let original = source_of(&harness, &id);
    let hold = id_of(&graph(&harness, &id), "hold");
    app(&mut harness).selections.insert(id.clone(), vec![hold]);
    settle(&mut harness);
    harness.step();

    alt(&mut harness, egui::Key::ArrowDown);
    let later = passes(&harness, &id);
    let order = steps(&harness, &id);
    assert_eq!(
        position(&order, "hold"),
        position(&order, "invite") + 1,
        "Alt+↓ moves it later: {order:?}"
    );
    undone(&mut harness, &id, &original);
    redone(&mut harness, &id, &later);

    alt(&mut harness, egui::Key::ArrowUp);
    passes(&harness, &id);
    assert_eq!(
        steps(&harness, &id),
        {
            let mut back = order.clone();
            let from = position(&back, "hold");
            let hold = back.remove(from);
            back.insert(from - 1, hold);
            back
        },
        "Alt+↑ moves it earlier"
    );

    // At the first place of its braces nothing moves, and the banner says
    // why.
    let second = id_of(&graph(&harness, &id), "second");
    app(&mut harness)
        .selections
        .insert(id.clone(), vec![second]);
    settle(&mut harness);
    harness.step();
    let text = source_of(&harness, &id);
    alt(&mut harness, egui::Key::ArrowUp);
    assert_eq!(source_of(&harness, &id), text);
    assert!(
        harness
            .query_by_label("`second` is already the first step in its braces.")
            .is_some()
    );
}

#[test]
fn diag06b_criterion_4_the_code_editor_keeps_its_keys() {
    let (_project, mut harness, id) = booking_day();
    harness.get_by_label("Split").click();
    settle(&mut harness);
    harness.step();
    let hold = id_of(&graph(&harness, &id), "hold");
    app(&mut harness).selections.insert(id.clone(), vec![hold]);
    settle(&mut harness);
    harness.step();
    let before = source_of(&harness, &id);
    harness.get_by_label_contains("Contents of").focus();
    settle(&mut harness);
    harness.step();
    assert!(harness.get_by_label_contains("Contents of").is_focused());
    for key in [egui::Key::ArrowDown, egui::Key::ArrowUp] {
        alt(&mut harness, key);
        assert_eq!(
            source_of(&harness, &id),
            before,
            "the board does not move a step while the editor has the keys"
        );
    }
}

// Design review -------------------------------------------------------------

/// Renders the window as it stands, the pointer and its hover kept, as
/// `<name>.png` in the capture directory.
fn held(harness: &mut FolderHarness, name: &str) {
    let image = harness
        .render()
        .unwrap_or_else(|error| panic!("cannot render {name}: {error}"));
    let directory = crate::testing::capture_dir();
    fs::create_dir_all(&directory).unwrap();
    image.save(directory.join(format!("{name}.png"))).unwrap();
}

/// `booking-day` at `size`, laid out by *Tidy up*.
fn tidied(size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, String) {
    let (project, mut harness, id) = booking_day();
    harness.set_size(size);
    settle(&mut harness);
    choose(&mut harness, "Tidy up");
    settle(&mut harness);
    harness.step();
    (project, harness, id)
}

/// The four views of DIAG-06b at 960 × 540 and 1600 × 1000: a wire
/// carried over a step that reads its source's step, and after its release, the notice
/// after a delete, the `+` on a line, and a step's menu.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_diag06b() {
    for (size, suffix) in [(MINIMUM, "960"), (WIDE, "1600")] {
        let (_project, mut harness, id) = shop_with(PAY);
        harness.set_size(size);
        settle(&mut harness);
        show(&mut harness, &id, &["begin", "pay"]);
        let target = id_of(&graph(&harness, &id), "pay");
        let from = centre(&harness, "Output __order of begin");
        let (socket, _) = board(&harness, &id).debug_order_sockets(&target);
        let to = screen(&harness, &id, socket.expect("an order socket"));
        press(&mut harness, from);
        move_to(&mut harness, from, to);
        harness.step();
        held(&mut harness, &format!("diag06b-refused-{suffix}"));
        release(&mut harness, to);
        settle(&mut harness);
        crate::testing::capture(
            &mut harness,
            &format!("diag06b-refused-after-{suffix}"),
            None,
        );
        drop(harness);

        let (_project, mut harness, id) = tidied(size);
        show(&mut harness, &id, &["equipment", "holidays"]);
        delete(&mut harness, &id, &["policy"]);
        crate::testing::capture(&mut harness, &format!("diag06b-notice-{suffix}"), None);
        drop(harness);

        let (_project, mut harness, id) = tidied(size);
        show(&mut harness, &id, &["first", "hold"]);
        let line = chain_line(&harness, &id, "first", "hold");
        let at = super::lay14::point_on(&harness, &id, &line);
        harness.hover_at(at);
        harness.step();
        harness.step();
        held(&mut harness, &format!("diag06b-plus-{suffix}"));
        drop(harness);

        let (_project, mut harness, id) = tidied(size);
        show(&mut harness, &id, &["hold"]);
        let card = board(&harness, &id)
            .debug_card_rect(&id_of(&graph(&harness, &id), "hold"))
            .expect("`hold` is drawn");
        click_at(
            &mut harness,
            card.center(),
            egui::PointerButton::Secondary,
            1,
        );
        harness.step();
        crate::testing::capture(&mut harness, &format!("diag06b-menu-{suffix}"), None);
    }
}

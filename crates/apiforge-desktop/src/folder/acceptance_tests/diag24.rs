//! DIAG-24 (#620) at the window: the menu of a card names the step a step
//! taken from it follows, its place: a step itself, for a check the step it
//! checks, for any other data card the last step that runs before it. A step
//! goes right after the place, unbound; a check or a value taken from a
//! step's menu is written right after that step. On `rooms/booking-day`,
//! through the gestures a person makes.
use super::diag06a::{graph, passes, show, steps};
use super::diag06b::{booking_day, indent, menu_of, position, redone, undone};
use super::diag07::{board, line_of};
use super::diag22::{
    COMPONENTS, bar, foot, full, headed, offered, places, popup, statement, take, with_sign_out,
    written_before,
};
use super::lay16::{centre, click_at, move_to, press, release};

use super::*;
use crate::testing::MINIMUM;

const SCENARIO: &str = "scenarios/booking-day.afs";

const REQUEST: &str = "  request = GET \"/endpoint\" {\n    timeout 10s\n  }\n";

/// `booking-day` with its checks drawn as cards.
fn with_checks() -> (tempfile::TempDir, FolderHarness, String) {
    let (project, mut harness, id) = booking_day();
    draw_checks(&mut harness);
    (project, harness, id)
}

fn draw_checks(harness: &mut FolderHarness) {
    app(harness).debug_draw_checks();
    settle(harness);
    choose(harness, "Tidy up");
    settle(harness);
    harness.step();
}

fn cleanup() -> (tempfile::TempDir, FolderHarness, String) {
    let (project, mut harness, id) = booking_day();
    harness.get_by_label("Cleanup").click();
    settle(&mut harness);
    choose(&mut harness, "Tidy up");
    settle(&mut harness);
    harness.step();
    (project, harness, id)
}

/// Right-clicks `handle`, whose menu names `place`, and takes `label`; the
/// new block's name.
fn from_menu(
    harness: &mut FolderHarness,
    id: &str,
    (handle, place): (&str, &str),
    label: &str,
) -> String {
    menu_of(harness, id, handle);
    headed(harness, place);
    take(harness, id, label).handle
}

/// The edit passes the check and gives `text`; one Undo gives `before` back.
fn wrote(harness: &mut FolderHarness, id: &str, before: &str, text: &str, case: &str) {
    assert_eq!(passes(harness, id), text, "{case}");
    undone(harness, id, before);
    redone(harness, id, text);
}

/// The top of the text `label` in the open menu, in view or not.
fn in_menu(harness: &FolderHarness, label: &str) -> Option<f32> {
    let menu = popup(harness);
    harness
        .query_all_by_label(label)
        .map(|node| node.rect())
        .filter(|rect| {
            menu.x_range().contains(rect.left())
                && menu.x_range().contains(rect.right())
                && rect.top() > menu.top()
        })
        .map(|rect| rect.top())
        .min_by(f32::total_cmp)
}

/// Where the menu's entries, its search and its *Data from* box stand.
fn top_of(harness: &FolderHarness, label: &str) -> f32 {
    in_menu(harness, label).unwrap_or_else(|| panic!("the menu holds {label}"))
}

fn next_caption(place: &str) -> String {
    format!("NEXT STEP · RUNS RIGHT AFTER {}", place.to_uppercase())
}

fn read_caption(card: &str) -> String {
    format!("READ {} · A CHECK OR A VALUE OF IT", card.to_uppercase())
}

fn yes_caption(card: &str) -> String {
    format!("IN THE YES BRANCH OF {}", card.to_uppercase())
}

/// The entries of the open menu that are of `ENTRIES`, from the top.
fn entries(harness: &FolderHarness) -> Vec<&'static str> {
    const ENTRIES: [&str; 8] = [
        "HTTP request",
        "IF / ELSE",
        "Value",
        "Comparison",
        "Data condition",
        "Object",
        "Variable",
        "Sign in",
    ];
    let mut found: Vec<(f32, &str)> = ENTRIES
        .into_iter()
        .filter_map(|label| Some((in_menu(harness, label)?, label)))
        .collect();
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.into_iter().map(|(_, label)| label).collect()
}

fn no_captions(harness: &FolderHarness) {
    for part in ["RUNS RIGHT AFTER", "A CHECK OR A VALUE OF IT", "IN THE YES"] {
        assert!(
            harness.query_by_label_contains(part).is_none(),
            "no caption {part}"
        );
    }
}

// Criterion 1 ----------------------------------------------------------------

#[test]
fn diag24_criterion_1_the_heading_names_the_place() {
    for (handle, place) in [
        ("slot", "begin"),
        ("big_enough", "team"),
        ("booked", "booked"),
        ("profile", "profile"),
        ("refused", "second"),
        ("not_booked", "first"),
    ] {
        let (_project, mut harness, id) = with_checks();
        menu_of(&mut harness, &id, handle);
        headed(&harness, place);
    }
}

/// From *Add a connected step* at the foot of a value's card.
#[test]
fn diag24_criterion_1_from_the_cards_button() {
    let (_project, mut harness, id) = booking_day();
    full(&mut harness);
    show(&mut harness, &id, &["slot"]);
    let button = centre(&harness, "Add a step connected to slot");
    click_at(&mut harness, button, egui::PointerButton::Primary, 1);
    harness.step();
    headed(&harness, "begin");
    let new = take(&mut harness, &id, "HTTP request").handle;
    let order = steps(&harness, &id);
    assert_eq!(position(&order, &new), position(&order, "begin") + 1);

    let (_project, mut harness, id) = booking_day();
    foot(&mut harness, &id, "profile");
    top_of(&harness, &next_caption("profile"));
}

// Criterion 2 ----------------------------------------------------------------

#[test]
fn diag24_criterion_2_a_step_goes_right_after_the_place() {
    for (handle, place, next) in [
        ("slot", "begin", Some("token")),
        ("big_enough", "team", Some("buildings")),
        ("booked", "booked", None),
    ] {
        let (_project, mut harness, id) = with_checks();
        let before = source_of(&harness, &id);
        let new = from_menu(&mut harness, &id, (handle, place), "HTTP request");
        let text = passes(&harness, &id);
        let order = steps(&harness, &id);
        match next {
            Some(next) => {
                assert_eq!(position(&order, &new), position(&order, place) + 1);
                assert_eq!(position(&order, next), position(&order, &new) + 1);
            }
            None => {
                assert_eq!(position(&order, &new), order.len() - 1, "{order:?}");
                let close = line_of(&text, "    not_booked = ") + 1;
                assert_eq!(
                    line_of(&text, &format!("{new} = ")),
                    close + 1,
                    "after its closing brace: {text}"
                );
            }
        }
        assert_eq!(format!("{}\n", statement(&text, &new)), REQUEST, "{handle}");
        assert!(!text.contains("@config"), "{handle}: {text}");
        if handle == "big_enough" {
            assert!(line_of(&text, "big_enough = ") < line_of(&text, &format!("{new} = ")));
        }
        // What *Add step* with the place selected gives on a fresh copy.
        let (_project, mut reference, other) = with_checks();
        bar(&mut reference, &other, &[place]);
        take(&mut reference, &other, "HTTP request");
        assert_eq!(text, source_of(&reference, &other), "{handle}");
        assert_eq!(order, steps(&reference, &other), "{handle}");
        assert_eq!(
            places(&graph(&harness, &id)),
            places(&graph(&reference, &other)),
            "{handle}: room is made alike"
        );
        undone(&mut harness, &id, &before);
        redone(&mut harness, &id, &text);
    }
}

// Criterion 3 ----------------------------------------------------------------

#[test]
fn diag24_criterion_3_in_braces() {
    let nested = "    request = GET \"/endpoint\" {\n      timeout 10s\n    }\n";
    for (handle, place, line, request) in [
        (
            "refused",
            "second",
            "    calendar = GET \"/rooms/{{rooms.body.items[0].id}}/bookings\" {\n",
            nested,
        ),
        (
            "not_booked",
            "first",
            "  hold = POST \"/payments/holds\" {\n",
            REQUEST,
        ),
    ] {
        let (_project, mut harness, id) = with_checks();
        let before = source_of(&harness, &id);
        let new = from_menu(&mut harness, &id, (handle, place), "HTTP request");
        let order = steps(&harness, &id);
        assert_eq!(position(&order, &new), position(&order, place) + 1);
        let text = written_before(&before, line, request);
        assert_eq!(indent(&text, &new), indent(&text, place), "{handle}");
        wrote(&mut harness, &id, &before, &text, handle);
    }
}

// Criterion 4 ----------------------------------------------------------------

#[test]
fn diag24_criterion_4_two_groups() {
    for size in [MINIMUM, WIDE] {
        for (handle, place) in [("profile", "profile"), ("slot", "begin")] {
            let (_project, mut harness, id) = booking_day();
            harness.set_size(size);
            settle(&mut harness);
            menu_of(&mut harness, &id, handle);
            let search = top_of(&harness, "Find an action");
            let (next, read) = (next_caption(place), read_caption(handle));
            let order = [
                next.as_str(),
                "HTTP request",
                "IF / ELSE",
                "Sign in",
                read.as_str(),
                "Data from",
                "Value",
                "Comparison",
                "Data condition",
                "Object",
                "Variable",
            ];
            let mut above = search;
            for label in order {
                let top = top_of(&harness, label);
                assert!(top > above, "{handle}: {label} stands lower");
                above = top;
            }
            // With a search typed, a group with no hit shows no caption.
            harness.event(egui::Event::Text("HTTP".to_owned()));
            harness.step();
            harness.step();
            assert!(in_menu(&harness, &next).is_some());
            assert!(in_menu(&harness, &read).is_none(), "{handle}");
            assert!(in_menu(&harness, "Data from").is_none(), "{handle}");
        }
    }
}

#[test]
fn diag24_criterion_4_on_an_if() {
    let (_project, mut harness, id) = booking_day();
    menu_of(&mut harness, &id, "booked");
    let (next, yes) = (next_caption("booked"), yes_caption("booked"));
    let order = [
        next.as_str(),
        "HTTP request",
        "IF / ELSE",
        "Sign in",
        yes.as_str(),
        "Value",
        "Comparison",
        "Data condition",
        "Object",
        "Variable",
    ];
    let mut above = top_of(&harness, "Find an action");
    for label in order {
        let top = top_of(&harness, label);
        assert!(top > above, "{label} stands lower");
        above = top;
    }
    assert!(in_menu(&harness, "Data from").is_none());
    // Only the second group has a hit.
    harness.event(egui::Event::Text("Comparison".to_owned()));
    harness.step();
    harness.step();
    assert!(in_menu(&harness, &next).is_none());
    assert!(in_menu(&harness, &yes).is_some());
}

#[test]
fn diag24_criterion_4_every_component_is_offered() {
    let (_project, mut harness, id) = with_sign_out();
    menu_of(&mut harness, &id, "slot");
    assert_eq!(offered(&mut harness), COMPONENTS, "the menu of `slot`");
    draw_checks(&mut harness);
    menu_of(&mut harness, &id, "big_enough");
    assert_eq!(offered(&mut harness), COMPONENTS, "of `big_enough`");
}

// Criterion 5 ----------------------------------------------------------------

#[test]
fn diag24_criterion_5_read_is_written_after_its_step() {
    let team = "  team = GET \"/teams/{{profile.body.team}}\" {\n";
    let calendar = "    calendar = GET \"/rooms/{{rooms.body.items[0].id}}/bookings\" {\n";
    for (handle, label, line, written) in [
        (
            "profile",
            "Comparison",
            team,
            "  check = assert profile.body == null\n",
        ),
        ("profile", "Value", team, "  value = profile.body\n"),
        (
            "profile",
            "Object",
            team,
            "  object = { field: profile.body }\n",
        ),
        (
            "profile",
            "Variable",
            team,
            "  var variable = profile.body\n",
        ),
        ("second", "Value", calendar, "    value = second.body\n"),
    ] {
        let (_project, mut harness, id) = booking_day();
        let before = source_of(&harness, &id);
        from_menu(&mut harness, &id, (handle, handle), label);
        let text = written_before(&before, line, written);
        wrote(&mut harness, &id, &before, &text, label);
    }
}

/// *Data from* names the output the block reads.
#[test]
fn diag24_criterion_5_the_chosen_output_is_read() {
    let (_project, mut harness, id) = booking_day();
    let before = source_of(&harness, &id);
    menu_of(&mut harness, &id, "profile");
    choose_output(&mut harness, "HTTP status · number");
    assert!(
        board(&harness, &id).debug_menu_open(),
        "the menu stays open"
    );
    take(&mut harness, &id, "Value");
    let text = written_before(
        &before,
        "  team = GET \"/teams/{{profile.body.team}}\" {\n",
        "  value = profile.status\n",
    );
    wrote(&mut harness, &id, &before, &text, "Value of the status");
}

/// Opens the *Data from* box and chooses the output listed as `label`.
fn choose_output(harness: &mut FolderHarness, label: &str) {
    let menu = popup(harness);
    let at = harness
        .query_all_by_role(egui::accesskit::Role::ComboBox)
        .map(|node| node.rect())
        .find(|rect| menu.contains_rect(*rect))
        .expect("the Data from box is in view")
        .center();
    click_at(harness, at, egui::PointerButton::Primary, 1);
    harness.step();
    let at = centre(harness, label);
    click_at(harness, at, egui::PointerButton::Primary, 1);
    harness.step();
}

// Criterion 6 ----------------------------------------------------------------

/// Pinned to what `main` wrote before this ticket, except the headings.
#[test]
fn diag24_criterion_6_other_menus_unchanged() {
    for (handle, line, written) in [
        ("slot", "  cleanup {\n", "  value = slot\n"),
        ("booked", "  } else {\n", "    value = \"\"\n"),
    ] {
        let (_project, mut harness, id) = booking_day();
        let before = source_of(&harness, &id);
        menu_of(&mut harness, &id, handle);
        take(&mut harness, &id, "Value");
        let text = written_before(&before, line, written);
        wrote(&mut harness, &id, &before, &text, handle);
    }

    let (_project, mut harness, id) = booking_day();
    let before = source_of(&harness, &id);
    menu_of(&mut harness, &id, "begin");
    headed(&harness, "begin");
    no_captions(&harness);
    assert!(in_menu(&harness, "Data from").is_none());
    assert_eq!(
        entries(&harness),
        [
            "HTTP request",
            "IF / ELSE",
            "Value",
            "Comparison",
            "Data condition",
            "Object",
            "Variable",
            "Sign in"
        ]
    );
    take(&mut harness, &id, "HTTP request");
    let line = "  token = sign_in@1(user: \"tester\")\n";
    let text = written_before(&before, line, REQUEST);
    wrote(&mut harness, &id, &before, &text, "from Start");
}

#[test]
fn diag24_criterion_6_the_menu_of_a_wire_unchanged() {
    for (label, written) in [
        (
            "HTTP request",
            "  @config({ method: \"GET\", timeoutMs: 10000.0 })\n  request = GET \"/endpoint\" {\n    \
             body profile.body\n    timeout 10s\n  }\n",
        ),
        ("Value", "  value = profile.body\n"),
    ] {
        let (_project, mut harness, id) = booking_day();
        let before = source_of(&harness, &id);
        show(&mut harness, &id, &["profile"]);
        let from = centre(&harness, "Output body of profile");
        let to = super::diag07::empty_point(&harness, &id);
        press(&mut harness, from);
        move_to(&mut harness, from, to);
        harness.step();
        release(&mut harness, to);
        harness.step();
        assert!(board(&harness, &id).debug_menu_open(), "the menu opens");
        headed(&harness, "profile");
        no_captions(&harness);
        assert!(top_of(&harness, "Data from") < top_of(&harness, "Find an action"));
        assert_eq!(
            entries(&harness),
            [
                "HTTP request",
                "IF / ELSE",
                "Value",
                "Comparison",
                "Data condition",
                "Object",
                "Variable"
            ]
        );
        take(&mut harness, &id, label);
        let text = written_before(&before, "  cleanup {\n", written);
        wrote(&mut harness, &id, &before, &text, label);
    }
}

// Criterion 7 ----------------------------------------------------------------

#[test]
fn diag24_criterion_7_cleanup() {
    let cancelled = "    cancelled = DELETE \"/bookings/{{first.body.id}}\" {\n";
    for (label, written) in [
        (
            "HTTP request",
            "    request = GET \"/endpoint\" {\n      timeout 10s\n    }\n",
        ),
        ("Value", "    value = released.body\n"),
    ] {
        let (_project, mut harness, id) = cleanup();
        let before = source_of(&harness, &id);
        from_menu(&mut harness, &id, ("released", "released"), label);
        let text = written_before(&before, cancelled, written);
        wrote(&mut harness, &id, &before, &text, label);
        if label == "HTTP request" {
            let order = steps(&harness, &id);
            assert_eq!(order, ["released", "request", "cancelled"], "{order:?}");
        }
    }
}

// Design review -------------------------------------------------------------

/// The menu of a request, a value, a check and an `if`, and the board after
/// a step and a read are added from each, at 960 × 540 and 1600 × 1000.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_diag24() {
    for (size, suffix) in [(MINIMUM, "960"), (WIDE, "1600")] {
        for handle in ["profile", "slot", "big_enough", "booked"] {
            for (label, name) in [("HTTP request", "step"), ("Comparison", "read")] {
                let (project, closed, _) = booking_day();
                drop(closed);
                let mut harness =
                    crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
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
                if handle == "big_enough" {
                    draw_checks(&mut harness);
                }
                menu_of(&mut harness, &id, handle);
                if name == "step" {
                    crate::testing::capture(
                        &mut harness,
                        &format!("diag24-menu-{handle}-{suffix}"),
                        None,
                    );
                }
                take(&mut harness, &id, label);
                crate::testing::capture(
                    &mut harness,
                    &format!("diag24-{name}-{handle}-{suffix}"),
                    None,
                );
            }
        }
    }
}

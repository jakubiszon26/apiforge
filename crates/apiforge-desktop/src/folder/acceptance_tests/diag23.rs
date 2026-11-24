//! DIAG-23 (#612) at the window: after a step is added or moved, the cards
//! that make room stand as *Tidy up* arranges them, measured from the step
//! they follow, so no card lands on one that stays; a kept card still in
//! the way sends them right, past it. Every other card keeps its position,
//! placed by hand or not. On `rooms/booking-day`, through the gestures a
//! person makes.
use super::diag06a::{graph, id_of, passes, rooms, show, steps, wire};
use super::diag06b::{booking_day, menu_of, position, redone, undone};
use super::diag07::board;
use super::lay16::{centre, click_at};
use super::*;
use crate::testing::MINIMUM;
use apiforge_core::format::{Graph, Position};
use apiforge_layout::GAP;
use egui::Rect;
use std::collections::BTreeMap;

const SCENARIO: &str = "scenarios/booking-day.afs";

/// The cards drawn on the board, by name, in board pixels, off screen too.
fn cards(harness: &FolderHarness, id: &str) -> Vec<(String, Rect)> {
    graph(harness, id)
        .nodes
        .iter()
        .filter_map(|node| {
            let rect = board(harness, id).debug_world(&node.id)?;
            Some((node.handle.clone(), rect))
        })
        .collect()
}

fn no_card_on_a_card(harness: &FolderHarness, id: &str, case: &str) {
    let cards = cards(harness, id);
    for (index, (one, a)) in cards.iter().enumerate() {
        for (other, b) in &cards[index + 1..] {
            assert!(
                !a.shrink(0.5).intersects(b.shrink(0.5)),
                "{case}: `{one}` {a:?} stands on `{other}` {b:?}"
            );
        }
    }
}

type Places = BTreeMap<String, (f64, f64)>;

fn places(graph: &Graph) -> Places {
    graph
        .nodes
        .iter()
        .map(|node| {
            let at = &node.position;
            (node.handle.clone(), (at.x, at.y))
        })
        .collect()
}

/// Where a whole *Tidy up* of the board would put its cards.
fn tidied(harness: &FolderHarness, id: &str) -> Places {
    places(&folder(harness).debug_tidied(id).expect("a tidied board"))
}

/// By how much `card` stands off the place the tidy gives it.
fn offset(places: &Places, tidied: &Places, card: &str) -> (f64, f64) {
    let ((x, y), (tidy_x, tidy_y)) = (places[card], tidied[card]);
    (x - tidy_x, y - tidy_y)
}

/// The steps from `first` on in the order they run, and the cards hung
/// under them.
fn from_on(harness: &FolderHarness, id: &str, first: &str) -> Vec<String> {
    let model = graph(harness, id);
    let order = steps(harness, id);
    let after = &order[position(&order, first)..];
    let plan = crate::canvas::shape::plan(&model, None).expect("a spine");
    let handle = |index: usize| model.nodes[index].handle.clone();
    let hung = plan
        .satellites
        .iter()
        .filter(|(_, under)| after.contains(&handle(plan.spine[*under])))
        .map(|(card, _)| handle(*card));
    after.iter().cloned().chain(hung).collect()
}

/// Every card from `first` on stands at the tidy's place plus one `offset`.
fn stand_off_the_tidy_by(harness: &FolderHarness, id: &str, first: &str, by: (f64, f64)) {
    let (now, tidy) = (places(&graph(harness, id)), tidied(harness, id));
    for card in from_on(harness, id, first) {
        assert_eq!(offset(&now, &tidy, &card), by, "`{card}`");
    }
}

/// Puts `card` at `at` by hand.
fn put(harness: &mut FolderHarness, id: &str, card: &str, at: (f64, f64)) {
    let moved = (
        id_of(&graph(harness, id), card),
        Position { x: at.0, y: at.1 },
    );
    app(harness).active_board_command(crate::canvas::Command::Move(vec![moved]));
    settle(harness);
    harness.step();
}

/// Moves `card` by hand.
fn drag(harness: &mut FolderHarness, id: &str, card: &str, by: (f64, f64)) {
    let (x, y) = places(&graph(harness, id))[card];
    put(harness, id, card, (x + by.0, y + by.1));
}

/// Turns *Compact cards* off and tidies the board.
fn full(harness: &mut FolderHarness) {
    app(harness).switch_density(crate::canvas::CardDensity::Full);
    settle(harness);
    choose(harness, "Tidy up");
    settle(harness);
    harness.step();
}

/// Types `label` into the open menu and takes its entry of that name; the
/// new step's name.
fn take(harness: &mut FolderHarness, id: &str, label: &str) -> String {
    assert!(board(harness, id).debug_menu_open(), "the menu is open");
    let before = graph(harness, id);
    harness.event(egui::Event::Text(label.to_owned()));
    harness.step();
    harness.step();
    let menu = harness
        .ctx
        .memory(|memory| memory.area_rect(egui::Id::new("board-action-menu")))
        .expect("the menu is drawn");
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
    assert_eq!(after.nodes.len(), before.nodes.len() + 1, "{label} added");
    after
        .nodes
        .iter()
        .find(|node| !before.nodes.iter().any(|old| old.id == node.id))
        .expect("the new step")
        .handle
        .clone()
}

/// *Add step* on the board's bar with `step` selected.
fn add_after(harness: &mut FolderHarness, id: &str, step: &str, label: &str) -> String {
    let selected = id_of(&graph(harness, id), step);
    app(harness)
        .selections
        .insert(id.to_owned(), vec![selected]);
    settle(harness);
    harness.step();
    harness.get_by_label("Add step").click();
    settle(harness);
    harness.step();
    take(harness, id, label)
}

/// Takes `label` from the menu of `step`.
fn from_menu(harness: &mut FolderHarness, id: &str, step: &str, label: &str) {
    menu_of(harness, id, step);
    let entry = centre(harness, label);
    click_at(harness, entry, egui::PointerButton::Primary, 1);
    settle(harness);
    harness.step();
}

/// The edit passes the check, puts no card on a card, and one Undo gives
/// the text and every position back.
fn made_room(harness: &mut FolderHarness, id: &str, before: (&str, &Places), case: &str) {
    let text = passes(harness, id);
    assert_ne!(text, before.0, "{case}: the text changes");
    no_card_on_a_card(harness, id, case);
    let after = places(&graph(harness, id));
    undone(harness, id, before.0);
    assert_eq!(&places(&graph(harness, id)), before.1, "{case}: Undo");
    redone(harness, id, &text);
    assert_eq!(places(&graph(harness, id)), after, "{case}: Redo");
}

// Criterion 1 ----------------------------------------------------------------

/// *Add step* with `profile` selected on the board as it opens.
#[test]
fn diag23_criterion_1_a_component_call() {
    for size in [MINIMUM, WIDE] {
        let (_project, mut harness, id) = booking_day();
        harness.set_size(size);
        settle(&mut harness);
        no_card_on_a_card(&harness, &id, "as opened");
        let before = (source_of(&harness, &id), places(&graph(&harness, &id)));
        add_after(&mut harness, &id, "profile", "Sign in");
        made_room(&mut harness, &id, (&before.0, &before.1), "Sign in");
    }
}

/// The same with *Compact cards* off and the board tidied: a request.
#[test]
fn diag23_criterion_1_at_full_density() {
    let (_project, mut harness, id) = booking_day();
    full(&mut harness);
    no_card_on_a_card(&harness, &id, "tidied at full density");
    let before = (source_of(&harness, &id), places(&graph(&harness, &id)));
    add_after(&mut harness, &id, "profile", "HTTP request");
    made_room(&mut harness, &id, (&before.0, &before.1), "HTTP request");
}

// Criterion 2 ----------------------------------------------------------------

#[test]
fn diag23_criterion_2_the_row_continues() {
    let (_project, mut harness, id) = booking_day();
    let before = places(&graph(&harness, &id));
    let new = add_after(&mut harness, &id, "profile", "Sign in");
    let (now, tidy) = (places(&graph(&harness, &id)), tidied(&harness, &id));
    assert_eq!(now["profile"], before["profile"], "`profile` stays");
    assert_eq!(now[&new].1, now["profile"].1, "in the row of `profile`");
    assert_eq!(now["team"].1, now["profile"].1, "in the row of `profile`");
    // One column on, as the tidy's row goes.
    for (one, other) in [("profile", new.as_str()), (new.as_str(), "team")] {
        assert_eq!(
            now[other].0 - now[one].0,
            tidy[other].0 - tidy[one].0,
            "`{other}` is one column on from `{one}`"
        );
        assert!(now[other].0 > now[one].0);
    }
    let by = offset(&now, &tidy, "profile");
    assert_ne!(by, (0.0, 0.0), "the tidy starts elsewhere");
    stand_off_the_tidy_by(&harness, &id, &new, by);
}

// Criterion 3 ----------------------------------------------------------------

/// The positions `main` gave before this ticket, on DIAG-06c's board, where
/// the tidy of the new board puts the step they follow where it stands.
const ADDED: [&str; 30] = [
    "begin 0 0",
    "big_enough 1420 190",
    "booked 1720 776",
    "buildings 1800 0",
    "calendar 400 1190",
    "equipment 1000 366",
    "first 0 776",
    "floors 0 366",
    "has_floors 0 556",
    "has_screen 1000 556",
    "held 900 990",
    "hold 900 776",
    "holidays 1800 366",
    "invite 1340 776",
    "invited 1340 990",
    "not_booked 0 1014",
    "notice 900 1190",
    "one_booking 400 1404",
    "policy 1420 366",
    "profile 1000 0",
    "refused 0 1404",
    "request 400 776",
    "rooms 500 366",
    "second 0 1190",
    "sent 900 1404",
    "short_enough 1420 556",
    "slot 0 156",
    "team 1420 0",
    "token 500 0",
    "working_day 1800 580",
];

/// `hold` after `invite`, whichever of the two is moved.
const MOVED: [&str; 29] = [
    "begin 0 0",
    "big_enough 1420 190",
    "booked 1440 776",
    "buildings 1800 0",
    "calendar 0 1190",
    "equipment 1000 366",
    "first 0 776",
    "floors 0 366",
    "has_floors 0 556",
    "has_screen 1000 556",
    "held 1020 990",
    "hold 1020 776",
    "holidays 1800 366",
    "invite 520 776",
    "invited 520 990",
    "not_booked 0 1014",
    "notice 520 1190",
    "one_booking 0 1404",
    "policy 1420 366",
    "profile 1000 0",
    "refused 1820 990",
    "rooms 500 366",
    "second 1820 776",
    "sent 520 1404",
    "short_enough 1420 556",
    "slot 0 156",
    "team 1420 0",
    "token 500 0",
    "working_day 1800 580",
];

#[test]
fn diag23_criterion_3_nothing_moves_where_the_frames_agree() {
    let (_project, mut harness, id) = rooms(SCENARIO, after_the_if);
    add_after(&mut harness, &id, "first", "HTTP request");
    assert_eq!(listed(&harness, &id), ADDED, "an add");

    let (_project, mut harness, id) = rooms(SCENARIO, after_the_if);
    wire(&mut harness, &id, "invite", "__order", "hold");
    assert_eq!(listed(&harness, &id), MOVED, "a move later");

    let (_project, mut harness, id) = rooms(SCENARIO, after_the_if);
    wire(&mut harness, &id, "first", "__order", "invite");
    assert_eq!(listed(&harness, &id), MOVED, "a move earlier");
}

/// Every card of the board with its position, in the order of their names.
fn listed(harness: &FolderHarness, id: &str) -> Vec<String> {
    places(&graph(harness, id))
        .iter()
        .map(|(card, (x, y))| format!("{card} {x} {y}"))
        .collect()
}

// Criterion 4 ----------------------------------------------------------------

#[test]
fn diag23_criterion_4_kept_cards_keep_their_places() {
    let (_project, mut harness, id) = booking_day();
    drag(&mut harness, &id, "token", (30.0, -70.0));
    drag(&mut harness, &id, "rooms", (30.0, 60.0));
    let before = places(&graph(&harness, &id));
    let new = add_after(&mut harness, &id, "profile", "Sign in");
    let (now, tidy) = (places(&graph(&harness, &id)), tidied(&harness, &id));
    for card in ["begin", "token", "profile", "slot"] {
        assert_eq!(now[card], before[card], "`{card}` keeps its place");
    }
    let by = offset(&now, &tidy, "profile");
    assert_eq!(offset(&now, &tidy, "rooms"), by, "`rooms` with the moved");
    stand_off_the_tidy_by(&harness, &id, &new, by);
    no_card_on_a_card(&harness, &id, "Sign in");
}

// Criterion 5 ----------------------------------------------------------------

#[test]
fn diag23_criterion_5_they_follow_the_step_they_follow() {
    let (_project, mut harness, id) = booking_day();
    drag(&mut harness, &id, "profile", (37.0, 37.0));
    let before = places(&graph(&harness, &id));
    let new = add_after(&mut harness, &id, "profile", "Sign in");
    let (now, tidy) = (places(&graph(&harness, &id)), tidied(&harness, &id));
    assert_eq!(now["profile"], before["profile"], "`profile` stays");
    let by = offset(&now, &tidy, "profile");
    stand_off_the_tidy_by(&harness, &id, &new, by);
}

// Criterion 6 ----------------------------------------------------------------

/// `slot` placed by hand where *Sign in* after `profile` puts `buildings`.
fn slot_in_the_way() -> (tempfile::TempDir, FolderHarness, String, (f64, f64)) {
    let (_project, mut reference, id) = booking_day();
    add_after(&mut reference, &id, "profile", "Sign in");
    let at = places(&graph(&reference, &id))["buildings"];
    let (project, mut harness, id) = booking_day();
    put(&mut harness, &id, "slot", at);
    (project, harness, id, at)
}

#[test]
fn diag23_criterion_6_a_kept_card_in_the_way() {
    let (_project, mut harness, id, at) = slot_in_the_way();
    add_after(&mut harness, &id, "profile", "Sign in");
    let now = places(&graph(&harness, &id));
    assert_eq!(now["slot"], at, "`slot` stands where it was put");
    no_card_on_a_card(&harness, &id, "`slot` in the way");
    let drawn: BTreeMap<String, Rect> = cards(&harness, &id).into_iter().collect();
    assert_eq!(
        drawn["buildings"].left(),
        drawn["slot"].right() + GAP,
        "one gap right of `slot`"
    );
    assert_eq!(now["buildings"].1, at.1, "in its row");
    let row = ["buildings", "floors", "rooms", "equipment", "policy"];
    let order = steps(&harness, &id);
    assert_eq!(
        order[position(&order, "buildings")..][..row.len()],
        row,
        "the second row"
    );
    for pair in row.windows(2) {
        assert_eq!(now[pair[0]].1, now[pair[1]].1, "{pair:?} in one row");
        assert!(
            drawn[pair[0]].right() < drawn[pair[1]].left(),
            "{pair:?} from left to right"
        );
    }
}

// Criterion 7 ----------------------------------------------------------------

#[test]
fn diag23_criterion_7_every_gesture() {
    type Gesture = fn(&mut FolderHarness, &str);
    let gestures: [(&str, bool, Gesture); 4] = [
        (
            "the `+` on the line after `profile`",
            false,
            |harness, id| {
                show(harness, id, &["profile", "team"]);
                let model = graph(harness, id);
                let (from, to) = (id_of(&model, "profile"), id_of(&model, "team"));
                let line = board(harness, id)
                    .debug_wires()
                    .iter()
                    .find(|wire| {
                        wire.chain
                            && wire.source.as_deref() == Some(from.as_str())
                            && wire.target == to
                    })
                    .expect("the line is drawn")
                    .id
                    .clone();
                let at = super::lay14::point_on(harness, id, &line);
                harness.hover_at(at);
                harness.step();
                harness.step();
                let plus = centre(harness, "Add a step between `profile` and `team`");
                click_at(harness, plus, egui::PointerButton::Primary, 1);
                harness.step();
                take(harness, id, "Sign in");
            },
        ),
        ("Move earlier", true, |harness, id| {
            from_menu(harness, id, "first", "Move earlier");
        }),
        ("a wire of the order", true, |harness, id| {
            wire(harness, id, "first", "__order", "holidays");
        }),
        ("a step from the menu of `profile`", false, |harness, id| {
            menu_of(harness, id, "profile");
            take(harness, id, "Sign in");
        }),
    ];
    for (case, at_full, gesture) in gestures {
        let (_project, mut harness, id) = booking_day();
        if at_full {
            full(&mut harness);
        }
        let before = (source_of(&harness, &id), places(&graph(&harness, &id)));
        gesture(&mut harness, &id);
        made_room(&mut harness, &id, (&before.0, &before.1), case);
    }
}

// In braces and in Cleanup ----------------------------------------------------

/// `booking-day` with `notice` written after the `if`: a card of Main that
/// stays when a step is added in the braces.
fn after_the_if(root: &std::path::Path) {
    let path = root.join(SCENARIO);
    let text = fs::read_to_string(&path).unwrap();
    let from = text.find("  notice = ").unwrap();
    let to = text.find("  booked = if").unwrap();
    let notice = text[from..to].to_owned();
    let close = "    not_booked = assert first.status == 201\n  }\n";
    assert_eq!(text.matches(close).count(), 1);
    let text = text
        .replace(&notice, "")
        .replace(close, &format!("{close}{notice}"));
    fs::write(path, text).unwrap();
}

/// The steps added in an `if` take the place of the step after it, which
/// stays: they go past it.
#[test]
fn diag23_in_braces() {
    for label in ["HTTP request", "Sign in"] {
        for step in ["second", "calendar"] {
            for at_full in [false, true] {
                let (_project, mut harness, id) = rooms(SCENARIO, after_the_if);
                if at_full {
                    full(&mut harness);
                }
                let before = (source_of(&harness, &id), places(&graph(&harness, &id)));
                add_after(&mut harness, &id, step, label);
                let case = format!("{label} after `{step}`, full density: {at_full}");
                let now = places(&graph(&harness, &id));
                assert_eq!(now["notice"], before.1["notice"], "{case}");
                made_room(&mut harness, &id, (&before.0, &before.1), &case);
            }
        }
    }
}

#[test]
fn diag23_in_cleanup() {
    for label in ["HTTP request", "Sign in"] {
        for step in ["released", "cancelled"] {
            for at_full in [false, true] {
                let (_project, mut harness, id) = booking_day();
                if at_full {
                    full(&mut harness);
                }
                harness.get_by_label("Cleanup").click();
                settle(&mut harness);
                choose(&mut harness, "Tidy up");
                settle(&mut harness);
                let before = (source_of(&harness, &id), places(&graph(&harness, &id)));
                let new = add_after(&mut harness, &id, step, label);
                let case = format!("{label} after `{step}`, full density: {at_full}");
                let now = places(&graph(&harness, &id));
                assert_eq!(now[&new].1, now[step].1, "{case}: in the row");
                made_room(&mut harness, &id, (&before.0, &before.1), &case);
            }
        }
    }
}

// Design review -------------------------------------------------------------

/// The board's cases after the add, the kept card in the way, a step in
/// braces and one in Cleanup, at both window sizes. Before each add `token`
/// (before the place) and `rooms` (after it) are placed by hand.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_diag23() {
    type Prepare = fn(&mut FolderHarness, &str);
    let hand: Prepare = |harness, id| {
        drag(harness, id, "token", (30.0, -70.0));
        drag(harness, id, "rooms", (30.0, 60.0));
    };
    let full_hand: Prepare = |harness, id| {
        full(harness);
        drag(harness, id, "token", (30.0, -70.0));
        drag(harness, id, "rooms", (30.0, 60.0));
    };
    let in_the_way: Prepare = |harness, id| {
        let (_project, mut reference, other) = booking_day();
        add_after(&mut reference, &other, "profile", "Sign in");
        let at = places(&graph(&reference, &other))["buildings"];
        put(harness, id, "slot", at);
    };
    let cleanup: Prepare = |harness, _| {
        harness.get_by_label("Cleanup").click();
        settle(harness);
        choose(harness, "Tidy up");
        settle(harness);
    };
    let nothing: Prepare = |_, _| {};
    let cases = [
        ("sign-in", hand, "profile", "Sign in"),
        ("request-full", full_hand, "profile", "HTTP request"),
        ("in-the-way", in_the_way, "profile", "Sign in"),
        ("in-braces", nothing, "second", "HTTP request"),
        ("cleanup", cleanup, "released", "Sign in"),
    ];
    for (size, suffix) in [(MINIMUM, "960"), (WIDE, "1600")] {
        for (case, prepare, step, label) in cases {
            let (project, closed, _) = rooms(SCENARIO, after_the_if_for(case));
            drop(closed);
            let mut harness =
                crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
            app(&mut harness).review_open(SCENARIO, None);
            settle(&mut harness);
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
            prepare(&mut harness, &id);
            add_after(&mut harness, &id, step, label);
            crate::testing::capture(&mut harness, &format!("diag23-{case}-{suffix}"), None);
            app(&mut harness).selections.insert(id.clone(), Vec::new());
            choose(&mut harness, "Fit the view");
            settle(&mut harness);
            crate::testing::capture(&mut harness, &format!("diag23-{case}-fit-{suffix}"), None);
        }
    }
}

fn after_the_if_for(case: &str) -> fn(&std::path::Path) {
    if case == "in-braces" {
        after_the_if
    } else {
        |_| {}
    }
}

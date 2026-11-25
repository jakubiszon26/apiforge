//! UX-43 (#623): plain Enter in the search of the step menu takes the first
//! step as it is drawn, as a click on it does; Escape closes an open popup
//! before it gives maximised Run results their height back.
use super::diag06a::graph;
use super::diag06b::{booking_day, menu_of, undone};
use super::diag07::board;
use super::diag12::{CHECKS, CHECKS_OF_FIELD, MENU, details, name_right_of, on_row, source};
use super::diag22::{bar, places, popup, take};
use super::ux05::maximised;
use super::*;
use egui::{Key, Modifiers};

const SEARCH: &str = "Find an action";

/// Types `text` in the open menu, whose search holds the keyboard.
fn search(harness: &mut FolderHarness, text: &str) {
    harness.event(egui::Event::Text(text.to_owned()));
    harness.step();
    harness.step();
}

fn enter(harness: &mut FolderHarness, modifiers: Modifiers) {
    harness.key_press_modifiers(modifiers, Key::Enter);
    settle(harness);
    harness.step();
}

/// Opens a step menu of `booking-day`: the bar's after `profile`, or the one
/// of the card `profile`.
fn opened(card: bool) -> (tempfile::TempDir, FolderHarness, String) {
    let (project, mut harness, id) = booking_day();
    if card {
        menu_of(&mut harness, &id, "profile");
    } else {
        bar(&mut harness, &id, &["profile"]);
    }
    assert!(board(&harness, &id).debug_menu_open());
    (project, harness, id)
}

/// The menu still stands open and nothing was added.
fn added_nothing(harness: &FolderHarness, id: &str, before: &str, case: &str) {
    assert!(
        board(harness, id).debug_menu_open(),
        "{case}: the menu stays open"
    );
    assert_eq!(source_of(harness, id), before, "{case}: nothing is added");
}

/// The menu still stands open, nothing was added and its search holds the
/// keyboard.
fn searches_on(harness: &FolderHarness, id: &str, before: &str, case: &str) {
    added_nothing(harness, id, before, case);
    assert!(
        harness.get_by_label(SEARCH).is_focused(),
        "{case}: the search keeps the keyboard"
    );
}

/// `on` is in the names or descriptions of *Value*, *Comparison*, *Data
/// condition* and *Sign in*: the bar's menu draws *Value* first, the menu of
/// a card *Sign in*, in the group of the next steps.
#[test]
fn ux43_criterion_1_enter_takes_the_first_step() {
    for (card, query, first) in [
        (false, "http", "HTTP request"),
        (false, "on", "Value"),
        (true, "http", "HTTP request"),
        (true, "on", "Sign in"),
        (true, "compar", "Comparison"),
    ] {
        let case = format!("card: {card}, {query}");
        let (_project, mut harness, id) = opened(card);
        take(&mut harness, &id, first);
        let clicked = source_of(&harness, &id);
        let places_clicked = places(&graph(&harness, &id));

        let (_project, mut harness, id) = opened(card);
        let before = source_of(&harness, &id);
        search(&mut harness, query);
        enter(&mut harness, Modifiers::NONE);
        assert!(
            !board(&harness, &id).debug_menu_open(),
            "{case}: the menu closes"
        );
        assert_eq!(source_of(&harness, &id), clicked, "{case}: as a click");
        assert_eq!(places(&graph(&harness, &id)), places_clicked, "{case}");
        undone(&mut harness, &id, &before);
    }
}

/// Empties the search, focused.
fn clear(harness: &mut FolderHarness) {
    harness.get_by_label(SEARCH).focus();
    harness.step();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.step();
    harness.key_press(Key::Backspace);
    harness.step();
}

#[test]
fn ux43_criterion_1_enter_without_a_step_row_adds_nothing() {
    for card in [false, true] {
        let (_project, mut harness, id) = opened(card);
        let before = source_of(&harness, &id);
        // Nothing typed, nothing found, only a variable or an operator found.
        for typed in ["", "  ", "zzz", "baseurl", "square root"] {
            clear(&mut harness);
            search(&mut harness, typed);
            enter(&mut harness, Modifiers::NONE);
            searches_on(&harness, &id, &before, &format!("card: {card}, {typed:?}"));
        }
        // Not a plain Enter.
        for modifiers in [Modifiers::SHIFT, Modifiers::ALT, Modifiers::COMMAND] {
            clear(&mut harness);
            search(&mut harness, "http");
            enter(&mut harness, modifiers);
            let case = format!("card: {card}, {modifiers:?}");
            added_nothing(&harness, &id, &before, &case);
        }
        // What is typed after an Enter that added nothing is searched for.
        clear(&mut harness);
        enter(&mut harness, Modifiers::NONE);
        search(&mut harness, "http");
        enter(&mut harness, Modifiers::NONE);
        assert!(!board(&harness, &id).debug_menu_open(), "card: {card}");
        assert_ne!(source_of(&harness, &id), before, "card: {card}");
    }
}

/// Enter on *Data from* opens its list, and on *Move earlier* moves the step:
/// neither adds what the search found.
#[test]
fn ux43_criterion_1_enter_on_another_control_adds_nothing() {
    let (_project, mut harness, id) = opened(true);
    let before = source_of(&harness, &id);
    search(&mut harness, "compar");
    let menu = popup(&harness);
    harness
        .query_all_by_role(egui::accesskit::Role::ComboBox)
        .find(|node| menu.contains_rect(node.rect()))
        .expect("the Data from box is in view")
        .focus();
    harness.step();
    enter(&mut harness, Modifiers::NONE);
    added_nothing(&harness, &id, &before, "Data from");
    assert!(egui::Popup::is_any_open(&harness.ctx), "its list opens");

    let (_project, mut harness, id) = booking_day();
    menu_of(&mut harness, &id, "buildings");
    let before = graph(&harness, &id).nodes.len();
    search(&mut harness, "http");
    harness.get_by_label("Move earlier").focus();
    harness.step();
    assert!(harness.get_by_label("Move earlier").is_focused());
    enter(&mut harness, Modifiers::NONE);
    assert_eq!(graph(&harness, &id).nodes.len(), before, "nothing is added");
}

/// Escape closes the popup `open` opened, then restores the panel.
fn escapes(harness: &mut FolderHarness, popup: &str) {
    assert!(maximised(harness), "{popup}: Run results are maximised");
    assert!(egui::Popup::is_any_open(&harness.ctx), "{popup}: opened");
    harness.key_press(Key::Escape);
    settle(harness);
    assert!(
        !egui::Popup::is_any_open(&harness.ctx),
        "{popup}: Escape closes it"
    );
    assert!(maximised(harness), "{popup}: the panel stays maximised");
    harness.key_press(Key::Escape);
    settle(harness);
    assert!(
        !maximised(harness),
        "{popup}: the second Escape restores the height"
    );
}

#[test]
fn ux43_criterion_2_escape_closes_the_popup_first() {
    let (_project, mut harness, id) = super::diag12::opened(WIDE, &source(CHECKS, ""));
    super::diag12::run(&mut harness);
    // A stale report offers no *Check*: the mark opens the field's checks.
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
    super::diag12::results_on(&mut harness, "cart");
    let row = name_right_of(&harness, "total", details(&harness));
    on_row(&harness, row, CHECKS_OF_FIELD)
        .expect("the mark on total is named for its checks")
        .click();
    settle(&mut harness);
    assert!(has_label(&harness, MENU), "the mark opens its menu");
    escapes(&mut harness, CHECKS_OF_FIELD);
    assert!(!has_label(&harness, MENU));

    // The policy of a fresh run.
    let (_project, mut harness, _) = super::diag12::opened(WIDE, &source(CHECKS, ""));
    super::diag12::run(&mut harness);
    super::diag12::results_on(&mut harness, "cart");
    harness.get_by_label("When a step fails").click();
    settle(&mut harness);
    escapes(&mut harness, "When a step fails");
}

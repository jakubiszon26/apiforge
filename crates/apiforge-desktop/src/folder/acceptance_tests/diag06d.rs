//! DIAG-06d (#493) at the window: an ordering wire dropped on an `if` moves
//! it with its braces; a step dropped behind a step of other braces goes
//! into those braces; a check takes a wire from *Yes* or *No* only, and
//! *Move out of the branch* in its card's menu writes it after the `if`. On
//! `rooms/booking-day`, through the gestures a person makes: a wire carried
//! from a socket found by its name to the step's order socket, and the
//! card's menu opened with the secondary button.
use super::diag06a::{graph, id_of, moved, passes, refused, rooms, show};
use super::diag07::{board, line_of};
use super::lay16::{centre, click_at};
use super::*;

const OWN_BRACES: &str = "A condition cannot go into its own braces.";
const ONLY_STEPS: &str = "Only steps are ordered. A check runs when what it reads exists.";
const MOVE_OUT: &str = "Move out of the branch";

fn booking_day() -> (tempfile::TempDir, FolderHarness, String) {
    rooms("scenarios/booking-day.afs", |_| {})
}

/// The line on which `handle` is written, with its indentation.
fn statement_line(text: &str, handle: &str) -> (usize, String) {
    let at = line_of(text, &format!("{handle} = "));
    (at, text.lines().nth(at).unwrap().to_owned())
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

fn position(order: &[String], handle: &str) -> usize {
    super::diag06a::at(order, handle)
}

// Criterion 1 ----------------------------------------------------------------

#[test]
fn diag06d_criterion_1() {
    let (_project, mut harness, id) = booking_day();
    let order = moved(&mut harness, &id, ("first", "__order", "booked"));
    assert_eq!(
        position(&order, "booked"),
        position(&order, "first") + 1,
        "{order:?}"
    );
    assert_eq!(position(&order, "second"), position(&order, "booked") + 1);
    assert_eq!(position(&order, "calendar"), position(&order, "second") + 1);
    assert!(
        position(&order, "hold") > position(&order, "calendar"),
        "{order:?}"
    );
    let text = passes(&harness, &id);
    let (booked, _) = statement_line(&text, "booked");
    let (second, line) = statement_line(&text, "second");
    let (hold, _) = statement_line(&text, "hold");
    assert!(booked < second && second < hold, "{text}");
    assert_eq!(indent(&line), 4, "`second` stays in the braces: {text}");
}

// Criterion 2 ----------------------------------------------------------------

#[test]
fn diag06d_criterion_2_into_a_branch() {
    let (_project, mut harness, id) = booking_day();
    let order = moved(&mut harness, &id, ("second", "__order", "notice"));
    assert_eq!(
        position(&order, "notice"),
        position(&order, "second") + 1,
        "{order:?}"
    );
    let text = passes(&harness, &id);
    let (_, line) = statement_line(&text, "notice");
    assert_eq!(indent(&line), 4, "`notice` stands in the braces: {text}");
    let (_, sent) = statement_line(&text, "sent");
    assert_eq!(indent(&sent), 4, "its check goes with it: {text}");
}

#[test]
fn diag06d_criterion_2_out_to_the_main_flow() {
    let (_project, mut harness, id) = booking_day();
    let order = moved(&mut harness, &id, ("notice", "__order", "calendar"));
    assert_eq!(
        position(&order, "calendar"),
        position(&order, "notice") + 1,
        "{order:?}"
    );
    assert_eq!(position(&order, "booked"), position(&order, "calendar") + 1);
    let text = passes(&harness, &id);
    let (_, line) = statement_line(&text, "calendar");
    assert_eq!(
        indent(&line),
        2,
        "`calendar` stands in the main flow: {text}"
    );
}

#[test]
fn diag06d_criterion_2_from_yes_to_no() {
    let (_project, mut harness, id) = booking_day();
    moved(&mut harness, &id, ("booked", "__false", "second"));
    let text = passes(&harness, &id);
    let (second, line) = statement_line(&text, "second");
    assert!(line_of(&text, "} else {") < second, "{text}");
    assert!(
        second < line_of(&text, "not_booked = "),
        "first in the braces: {text}"
    );
    assert_eq!(indent(&line), 4, "{text}");
}

#[test]
fn diag06d_criterion_2_an_if_into_its_own_braces() {
    let (_project, mut harness, id) = booking_day();
    refused(
        &mut harness,
        &id,
        ("second", "__order", "booked"),
        OWN_BRACES,
    );
}

// Criterion 5 ----------------------------------------------------------------

#[test]
fn diag06d_criterion_5_a_check_into_a_branch_from_yes() {
    let (_project, mut harness, id) = booking_day();
    moved(&mut harness, &id, ("booked", "__true", "sent"));
    let text = passes(&harness, &id);
    let (sent, line) = statement_line(&text, "sent");
    assert!(line_of(&text, "booked = ") < sent, "{text}");
    assert!(
        sent < line_of(&text, "second = "),
        "first in the braces: {text}"
    );
    assert_eq!(indent(&line), 4, "{text}");
}

#[test]
fn diag06d_criterion_5_a_check_takes_no_wire_from_next() {
    let (_project, mut harness, id) = booking_day();
    refused(&mut harness, &id, ("first", "__order", "sent"), ONLY_STEPS);
}

#[test]
fn diag06d_criterion_5_move_out_of_the_branch() {
    let (_project, mut harness, id) = booking_day();
    let before = source_of(&harness, &id);
    show(&mut harness, &id, &["refused"]);
    let card = board(&harness, &id)
        .debug_card_rect(&id_of(&graph(&harness, &id), "refused"))
        .expect("`refused` is drawn");
    click_at(
        &mut harness,
        card.center(),
        egui::PointerButton::Secondary,
        1,
    );
    harness.step();
    let entry = centre(&harness, MOVE_OUT);
    click_at(&mut harness, entry, egui::PointerButton::Primary, 1);
    settle(&mut harness);
    harness.step();
    let text = passes(&harness, &id);
    assert_ne!(text, before, "the check moves");
    let (refused_at, line) = statement_line(&text, "refused");
    assert_eq!(indent(&line), 2, "after the `if`'s braces: {text}");
    assert!(line_of(&text, "not_booked = ") < refused_at, "{text}");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines[refused_at - 1].trim(),
        "}",
        "directly after the closing brace: {text}"
    );
    // A check in the main flow has no such entry.
    let (_project, mut harness, id) = booking_day();
    show(&mut harness, &id, &["sent"]);
    let card = board(&harness, &id)
        .debug_card_rect(&id_of(&graph(&harness, &id), "sent"))
        .expect("`sent` is drawn");
    click_at(
        &mut harness,
        card.center(),
        egui::PointerButton::Secondary,
        1,
    );
    harness.step();
    assert!(harness.query_by_label(MOVE_OUT).is_none());
}

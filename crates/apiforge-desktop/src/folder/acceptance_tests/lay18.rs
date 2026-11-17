//! MOCK (LAY-18, #302): the views of the mock-up. Throwaway, not for merge.
use super::lay16::{press, release};
use super::lay23::{Checks, board, id_of, show, tidied};
use super::*;
use crate::canvas::Mock18;
use crate::testing::MINIMUM;
use egui::{Pos2, Rect};

fn board_mut<'a>(harness: &'a mut FolderHarness, id: &str) -> &'a mut crate::canvas::Canvas {
    app(harness)
        .canvases
        .get_mut(id)
        .expect("the board is drawn")
}

fn mock(harness: &mut FolderHarness, id: &str, mock: Mock18) {
    board_mut(harness, id).mock18 = mock;
    harness.step();
    harness.step();
}

/// A capture that keeps the pointer where it is.
fn save(harness: &mut FolderHarness, name: &str) {
    harness.step();
    let image = harness
        .render()
        .unwrap_or_else(|error| panic!("cannot render {name}: {error}"));
    let directory = crate::testing::capture_dir();
    fs::create_dir_all(&directory).unwrap();
    image.save(directory.join(format!("{name}.png"))).unwrap();
}

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

/// The count at `handle`'s port on screen.
fn count(harness: &FolderHarness, id: &str, handle: &str) -> Rect {
    let node = id_of(harness, id, handle);
    let canvas = board(harness, id);
    let count = canvas
        .debug_counts()
        .iter()
        .find(|count| count.count.source == node)
        .unwrap_or_else(|| panic!("a count at {handle}"));
    eprintln!(
        "COUNT {handle}: {} ({} readers)",
        count.count.label, count.count.readers
    );
    canvas.debug_to_screen(count.world)
}

#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay18() {
    for (size, suffix) in [(WIDE, "1600"), (MINIMUM, "960")] {
        let (_project, mut harness, id) = tidied("shop-30", size, Checks::Collapsed);
        assert!(board(&harness, &id).debug_compact(), "Compact is the default");
        for count in board(&harness, &id).debug_counts() {
            eprintln!("ALL COUNTS: {}", count.count.label);
        }
        // The board as it opens.
        save(&mut harness, &format!("lay18-board-before-{suffix}"));
        for (step, zoom, tag) in [("login", 0.6, "login"), ("cart", 1.0, "cart-100")] {
            show(&mut harness, &id, step, zoom);
            let at = count(&harness, &id, step).center();

            // Main today.
            away(&mut harness);
            save(&mut harness, &format!("lay18-{tag}-rest-before-{suffix}"));
            hover(&mut harness, at);
            save(&mut harness, &format!("lay18-{tag}-point-before-{suffix}"));
            click(&mut harness, at);
            away(&mut harness);
            save(&mut harness, &format!("lay18-{tag}-click-before-{suffix}"));
            escape(&mut harness);

            // Option A: the list at the count.
            mock(&mut harness, &id, Mock18::List);
            hover(&mut harness, at);
            save(&mut harness, &format!("lay18-{tag}-point-a-list-{suffix}"));
            click(&mut harness, at);
            away(&mut harness);
            save(&mut harness, &format!("lay18-{tag}-click-a-list-{suffix}"));
            escape(&mut harness);

            // Option B: a count per field.
            mock(&mut harness, &id, Mock18::PerField);
            away(&mut harness);
            save(&mut harness, &format!("lay18-{tag}-rest-b-fields-{suffix}"));
            mock(&mut harness, &id, Mock18::Off);
        }
    }
}

//! LAY-27 (#344): a collapsed check line carries the checks' numbers. The
//! line of a step whose checks are folded starts with their numbers in the
//! engine's order, `7 · 1 check`, `7–8 · 2 checks`, `21, 23 · 2 checks`,
//! then reads as before; a pill wider than its card reads the numbers alone,
//! and below 45 %, where a card's pill has no room, the numbers stand muted
//! at the right end of the name's row (Q1 = A). A check with two subjects is
//! named in both lines. On `far-value` and `shop-30`.
use super::lay16::{
    SCENARIO, canvas, document, graph, id_of, shop_30, show, switch_density, world,
};
use super::*;
use crate::canvas::{DrawnText, TextKind};
use crate::theme;
use apiforge_core::{
    format::{Graph, Node},
    graph::sequence,
    run::Phase,
};
use egui::{Color32, FontId, Rect};
use std::collections::{BTreeSet, HashMap};
use std::time::{Duration, Instant};

const TIDY: &str = "Tidy up";
const FIT: &str = "Fit the view";

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios/lay")
        .join(format!("{name}.afs"));
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the fixture {}: {error}", path.display()))
}

/// `source` on the board of the demo shop as a new install opens it:
/// compact cards, checks collapsed.
fn open(source: &str, size: egui::Vec2) -> (tempfile::TempDir, FolderHarness) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), source).unwrap();
    let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    (project, harness)
}

/// The same without positions, tidied up with nothing selected.
fn tidied(name: &str, size: egui::Vec2) -> (tempfile::TempDir, FolderHarness) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), fixture(name)).unwrap();
    fs::remove_file(project.path().join(".apiforge/diagram.json")).unwrap();
    let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let id = document(&harness);
    app(&mut harness).selections.insert(id, Vec::new());
    settle(&mut harness);
    choose(&mut harness, TIDY);
    harness.step();
    (project, harness)
}

fn fit(harness: &mut FolderHarness) {
    choose(harness, FIT);
    harness.step();
}

fn simulate(harness: &mut FolderHarness) {
    choose(harness, "Run simulation");
    let deadline = Instant::now() + Duration::from_secs(30);
    while folder(harness).runs.handle.is_some() {
        assert!(Instant::now() < deadline, "the run must finish");
        harness.step();
        std::thread::sleep(Duration::from_millis(5));
    }
    settle(harness);
    harness.step();
}

/// *Checks* on the board's bar (LAY-30), which folds or draws every check.
fn switch_checks(harness: &mut FolderHarness) {
    harness.get_by_label("Checks").click();
    settle(harness);
    harness.step();
}

/// The Cleanup graph of the scenario.
fn cleanup_graph(harness: &FolderHarness) -> Graph {
    let snapshot = folder(harness).snapshot(&document(harness)).unwrap();
    graph_of(snapshot.model.as_ref().unwrap(), true).clone()
}

/// Every step's place in the engine's order, from 1, by handle.
fn order(graph: &Graph) -> HashMap<String, usize> {
    sequence(graph)
        .into_iter()
        .enumerate()
        .map(|(index, id)| {
            let node = graph.nodes.iter().find(|node| node.id == id).unwrap();
            (node.handle.clone(), index + 1)
        })
        .collect()
}

/// The checks line of `handle`'s card in the last frame.
fn line(harness: &FolderHarness, handle: &str) -> String {
    canvas(harness)
        .debug_aggregate(&id_of(harness, handle))
        .unwrap_or_else(|| panic!("{handle} has no checks line"))
}

/// The numbers as the owner decided them, written here apart from the board:
/// ascending, a run of two or more as a range with an en dash.
fn written(numbers: &[usize]) -> String {
    let mut numbers = numbers.to_vec();
    numbers.sort_unstable();
    let mut parts: Vec<String> = Vec::new();
    let mut start = 0;
    for index in 0..numbers.len() {
        if index + 1 == numbers.len() || numbers[index + 1] != numbers[index] + 1 {
            parts.push(if index > start {
                format!("{}–{}", numbers[start], numbers[index])
            } else {
                numbers[start].to_string()
            });
            start = index + 1;
        }
    }
    parts.join(", ")
}

/// The items of a list of numbers (`7–9, 12`, `21, …`), or `None` when the
/// text is not one: a range is one item.
fn items(text: &str) -> Option<Vec<String>> {
    let items: Vec<String> = text.split(", ").map(str::to_owned).collect();
    let well_formed = items.iter().enumerate().all(|(index, item)| {
        (item == "…" && index + 1 == items.len() && index > 0)
            || item.split('–').count() <= 2
                && item
                    .split('–')
                    .all(|number| !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()))
    });
    (well_formed && !text.is_empty()).then_some(items)
}

/// The numbers a list of numbers names, its ranges expanded.
fn expand(items: &[String]) -> Vec<usize> {
    items
        .iter()
        .filter(|item| *item != "…")
        .flat_map(|item| {
            let mut ends = item
                .split('–')
                .map(|number| number.parse::<usize>().unwrap());
            let first = ends.next().unwrap();
            let last = ends.next().unwrap_or(first);
            first..=last
        })
        .collect()
}

/// Every number a text names in its statements: the checks line, a pill or
/// the numbers on a name's row.
fn numbers_in(text: &str) -> Vec<usize> {
    text.split(" · ")
        .filter_map(items)
        .flat_map(|items| expand(&items))
        .collect()
}

/// The list of numbers a checks line starts with, as items.
fn leading(text: &str) -> Option<Vec<String>> {
    items(text.split(" · ").next()?)
}

/// The width of `text` in `font`, as the board lays it out.
fn width(harness: &FolderHarness, text: &str, font: FontId) -> f32 {
    harness.ctx.fonts_mut(|fonts| {
        fonts
            .layout_no_wrap(text.to_owned(), font, Color32::BLACK)
            .size()
            .x
    })
}

/// How much of a name the board shows before its ellipsis, on screen: all
/// of it when it is not cut.
fn shown_before_ellipsis(harness: &FolderHarness, name: &DrawnText) -> f32 {
    harness.ctx.fonts_mut(|fonts| {
        // As the board's label lays out a line it may cut.
        let mut job = egui::text::LayoutJob::simple_singleline(
            name.text.clone(),
            FontId::new(name.size, theme::bold_family()),
            Color32::BLACK,
        );
        job.wrap.max_width = name.rect.width();
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        let galley = fonts.layout_job(job);
        galley.rows[0]
            .glyphs
            .iter()
            .filter(|glyph| glyph.chr != '…')
            .map(|glyph| glyph.max_x())
            .fold(0.0, f32::max)
    })
}

/// The cards wholly in view in the last frame, with what they wrote.
fn in_view(harness: &FolderHarness) -> Vec<(String, Rect, Vec<DrawnText>)> {
    let board = canvas(harness);
    let view = board.debug_focus();
    board
        .debug_cards()
        .iter()
        .filter(|(_, rect)| view.contains_rect(*rect))
        .map(|(id, rect)| (id.clone(), *rect, board.debug_texts(id)))
        .collect()
}

/// The numbers written on the board in the last frame: those of the badges
/// and those of the checks lines, pills and name rows, of the cards in view.
fn on_board(harness: &FolderHarness) -> (Vec<usize>, Vec<usize>) {
    let board = canvas(harness);
    let mut badges = Vec::new();
    let mut lines = Vec::new();
    for (id, _, texts) in in_view(harness) {
        if let Some(badge) = board.debug_badge(&id) {
            badges.push(badge.text.parse::<usize>().expect("a badge's number"));
        }
        for text in texts.iter().filter(|text| text.kind == TextKind::Result) {
            lines.extend(numbers_in(&text.text));
        }
    }
    (badges, lines)
}

/// Every number from 1 to the number of steps is on the board, as a badge
/// or in a line, and none is both.
fn assert_every_number_once(harness: &FolderHarness, context: &str) {
    let steps = sequence(&graph(harness)).len();
    let (badges, lines) = on_board(harness);
    let badges: BTreeSet<usize> = badges.into_iter().collect();
    let lines: BTreeSet<usize> = lines.into_iter().collect();
    assert!(
        badges.is_disjoint(&lines),
        "{context}: a number is both a badge and in a line: {:?}",
        badges.intersection(&lines).collect::<Vec<_>>()
    );
    let all: BTreeSet<usize> = badges.union(&lines).copied().collect();
    assert_eq!(
        all,
        (1..=steps).collect::<BTreeSet<usize>>(),
        "{context}: every number from 1 to {steps}, badges {badges:?}, lines {lines:?}"
    );
}

/// What a card at Overview did with the numbers of its folded checks.
#[derive(Default)]
struct Seen {
    /// Cards that wrote them at the right end of the name's row.
    rows: usize,
    /// Cards whose pill read them, before the words or alone.
    pills: usize,
    /// Cards with folded checks that wrote no name.
    nameless: usize,
}

/// Criterion 7, on every card in view below 45 %: a card that writes its
/// name and whose pill has no room writes the numbers, whole and muted (red
/// where a check failed), at least 9 px high, at the right end of the name's
/// row, over no badge, chip, count, mark or port; the name keeps 22 px
/// before its ellipsis, or the list loses numbers from its end and ends with
/// `…`, the first kept. A pill reads the numbers before the words, or alone
/// when the text is wider than the card. A card that writes no name gets
/// nothing.
fn assert_numbers_below_45(harness: &FolderHarness, context: &str) -> Seen {
    let board = canvas(harness);
    let zoom = board.debug_zoom();
    assert!(zoom < 0.45, "{context}: below 45 %, not {zoom}");
    let marked = board.debug_marked();
    let mut obstacles: Vec<(String, Rect)> = Vec::new();
    for (id, _) in board.debug_cards() {
        if let Some(badge) = board.debug_badge(id) {
            obstacles.push((format!("the badge of {id}"), badge.rect));
        }
    }
    for tag in board.debug_tags() {
        obstacles.push((format!("the chip {}", tag.shown), tag.rect));
    }
    for count in board.debug_counts() {
        obstacles.push(("a count".to_owned(), board.debug_to_screen(count.world)));
    }
    for mark in board.debug_var_marks() {
        obstacles.push(("a + var mark".to_owned(), mark.rect));
    }
    // Every socket is centred on its card's edge.
    let socket = (4.5 * zoom).max(1.25) + 0.5;
    let mut seen = Seen::default();
    for (id, card, texts) in in_view(harness) {
        let Some(full) = board.debug_aggregate(&id) else {
            continue;
        };
        let Some(parts) = leading(&full) else {
            continue;
        };
        let failed = marked.contains(&id);
        // The count of the steps a failure kept from running, under the
        // pill (LAY-19), is not the checks' numbers.
        let results: Vec<&DrawnText> = texts
            .iter()
            .filter(|text| text.kind == TextKind::Result && !text.text.ends_with(" not run"))
            .collect();
        let Some(name) = texts.iter().find(|text| text.kind == TextKind::Name) else {
            assert!(
                results.is_empty(),
                "{context}, {id}: a card that writes no name gets nothing: {results:?}"
            );
            seen.nameless += 1;
            continue;
        };
        let row: Vec<&&DrawnText> = results
            .iter()
            .filter(|text| {
                let middle = text.rect.center().y;
                name.rect.top() <= middle && middle <= name.rect.bottom()
            })
            .collect();
        let pills: Vec<&&DrawnText> = results
            .iter()
            .filter(|text| text.rect.center().y > name.rect.bottom())
            .collect();
        assert_eq!(
            row.len() + pills.len(),
            1,
            "{context}, {id}: the numbers once, on the name's row or in the pill: {texts:?}"
        );
        if let Some(numbers) = row.first() {
            seen.rows += 1;
            let shown = items(&numbers.text).unwrap_or_else(|| {
                panic!("{context}, {id}: whole numbers on the name's row: {numbers:?}")
            });
            let kept: Vec<&String> = shown.iter().filter(|item| *item != "…").collect();
            assert!(!kept.is_empty(), "{context}, {id}: the first number kept");
            assert!(
                kept.iter().zip(&parts).all(|(shown, part)| *shown == part),
                "{context}, {id}: {:?} is the start of {parts:?}",
                numbers.text
            );
            assert_eq!(
                kept.len() < parts.len(),
                shown.last().is_some_and(|item| item == "…"),
                "{context}, {id}: a shortened list ends with …: {:?}",
                numbers.text
            );
            assert_eq!(
                numbers.color,
                if failed { theme::DANGER } else { theme::MUTED },
                "{context}, {id}: muted, red where a check failed"
            );
            assert!(numbers.size >= 9.0, "{context}, {id}: {} px", numbers.size);
            let whole = width(harness, &numbers.text, FontId::proportional(numbers.size));
            assert!(
                numbers.rect.width() + 0.25 >= whole,
                "{context}, {id}: no digit is cut, {} of {whole} px shown",
                numbers.rect.width()
            );
            assert!(
                card.shrink(socket).contains_rect(numbers.rect),
                "{context}, {id}: the numbers {:?} inside {card:?}, clear of its ports",
                numbers.rect
            );
            assert!(
                card.right() - numbers.rect.right() <= 6.0
                    && numbers.rect.left() >= name.rect.right(),
                "{context}, {id}: the numbers {:?} at the right end of the name's row, after \
                 the name {:?}",
                numbers.rect,
                name.rect
            );
            for (what, rect) in &obstacles {
                assert!(
                    !rect.intersects(numbers.rect),
                    "{context}, {id}: the numbers {:?} over {what} {rect:?}",
                    numbers.rect
                );
            }
            // The name keeps 22 px before its ellipsis, unless the first
            // number alone takes the room.
            let before = shown_before_ellipsis(harness, name);
            let first = if parts.len() > 1 {
                format!("{}, …", parts[0])
            } else {
                parts[0].clone()
            };
            assert!(
                before >= 22.0 || numbers.text == first,
                "{context}, {id}: the name keeps {before} px before its ellipsis beside {:?}",
                numbers.text
            );
        }
        if let Some(pill) = pills.first() {
            seen.pills += 1;
            let numbers = parts.join(", ");
            let size = FontId::proportional(pill.size);
            if pill.text == numbers {
                assert!(
                    width(harness, &full, size) + 10.0 > card.width() - 10.0,
                    "{context}, {id}: the numbers alone only when {full:?} is wider than \
                     the card"
                );
            } else if pill.text != full && pill.text.ends_with("1 check") {
                // A written check gives way to the count where it is wider
                // than the card (LAY-30).
                assert!(
                    width(harness, &full, size.clone()) + 10.0 > card.width() - 10.0,
                    "{context}, {id}: the count for {full:?} only when it is wider than the card"
                );
                assert!(pill.text.starts_with(&format!("{numbers} · ")));
            } else {
                assert_eq!(pill.text, full, "{context}, {id}: the pill reads the line");
                assert!(full.starts_with(&format!("{numbers} · ")));
            }
            assert_eq!(
                pill.color,
                if failed { theme::DANGER } else { theme::MUTED },
                "{context}, {id}: the pill in the line's colour"
            );
        }
    }
    seen
}

fn node_of<'a>(graph: &'a Graph, id: &str) -> &'a Node {
    graph.nodes.iter().find(|node| node.id == id).unwrap()
}

/// Criterion 2: `far-value` at 1600 × 1000 after *Fit the view*: the lines
/// of `quantity`, `checkout`, `lookup` and `cart`, and every number from 1
/// to the number of steps on the board, as a badge or in a line, never both.
#[test]
fn lay27_criterion_2_far_value_names_its_folded_checks() {
    let (_project, mut harness) = tidied("far-value", WIDE);
    fit(&mut harness);
    let order = order(&graph(&harness));
    assert_eq!(
        (
            order["quantity_ok"],
            order["same_item"],
            order["checkout_ok"],
            order["lookup_ok"]
        ),
        (7, 19, 21, 23),
        "the engine's order"
    );
    // A step with one check writes it (LAY-30); a Data condition is
    // counted.
    assert_eq!(line(&harness, "quantity"), "7 · status == 200");
    assert_eq!(line(&harness, "checkout"), "21, 23 · 2 checks");
    assert_eq!(line(&harness, "lookup"), "23 · 1 check");
    // `same_item` compares `view` with `cart`'s product: under both, each
    // leaving out its own name.
    assert_eq!(
        line(&harness, "cart"),
        "19 · view.body.items[0].id == productId"
    );
    assert_eq!(
        line(&harness, "view"),
        "19 · body.items[0].id == cart.productId"
    );
    let lines: Vec<String> = in_view(&harness)
        .into_iter()
        .flat_map(|(_, _, texts)| texts)
        .filter(|text| text.kind == TextKind::Result)
        .map(|text| text.text)
        .collect();
    for written in ["7 · status == 200", "21, 23 · 2 checks", "23 · 1 check"] {
        assert!(
            lines.iter().any(|line| line == written),
            "{written} is on the board: {lines:?}"
        );
    }
    assert_every_number_once(&harness, "far-value at Fit, 1600 × 1000");
}

/// Criterion 3: on `shop-30` two consecutive folded checks read a range
/// (`login`); `receipt_total`, inside a branch of `paid`, is named by its
/// place in the engine's order, in the lines of both its subjects.
#[test]
fn lay27_criterion_3_ranges_branches_and_two_subjects() {
    let (_project, mut harness) = open(&shop_30(), WIDE);
    show(&mut harness, "cart", 0.6);
    let order = order(&graph(&harness));
    let login = written(&[order["logged_in"], order["known_user"]]);
    assert!(login.contains('–'), "a range: {login}");
    assert_eq!(line(&harness, "login"), format!("{login} · 2 checks"));
    let receipt_total = order["receipt_total"];
    // Its one check is written out (LAY-30).
    assert_eq!(
        line(&harness, "receipt"),
        format!("{receipt_total} · body.total ~= cart.body.total within 0.01")
    );
    assert_eq!(
        line(&harness, "cart"),
        format!(
            "{} · 3 checks",
            written(&[order["cart_created"], order["cart_total"], receipt_total])
        )
    );
}

/// Criterion 4: after a simulation of both fixtures every number in a line
/// is the row of that check in Run results, and the line keeps its state
/// after the numbers, red with the red border where a check failed.
#[test]
fn lay27_criterion_4_the_numbers_are_the_rows_of_run_results() {
    for (name, source) in [("far-value", fixture("far-value")), ("shop-30", shop_30())] {
        let (_project, mut harness) = open(&source, WIDE);
        simulate(&mut harness);
        let run = folder(&harness)
            .fresh_run(&document(&harness))
            .expect("a fresh run");
        let rows: Vec<String> = run
            .steps
            .iter()
            .filter(|step| step.phase == Phase::Main)
            .map(|step| step.node_id.clone())
            .collect();
        let graph = graph(&harness);
        let hidden = canvas(&harness).debug_hidden().to_vec();
        let mut named = 0;
        for subject in &graph.nodes {
            let Some(text) = canvas(&harness).debug_aggregate(&subject.id) else {
                continue;
            };
            for number in leading(&text)
                .map(|items| expand(&items))
                .unwrap_or_default()
            {
                let check = &rows[number - 1];
                assert!(
                    hidden.contains(check),
                    "{name}: {number} in {}'s line is the folded {check}",
                    subject.handle
                );
                assert!(
                    node_of(&graph, check)
                        .inputs
                        .values()
                        .any(|binding| binding.references_node(&subject.id)),
                    "{name}: row {number}, {check}, is a check of {}",
                    subject.handle
                );
                named += 1;
            }
        }
        assert!(named >= 5, "{name}: numbers named: {named}");
        let order = order(&graph);
        if name == "far-value" {
            // The fixture's simulation stops at `quantity`; the lines keep
            // the state of their checks after the numbers.
            for (handle, numbers) in [("quantity", "7"), ("checkout", "21, 23")] {
                let text = line(&harness, handle);
                let state = text
                    .strip_prefix(&format!("{numbers} · "))
                    .unwrap_or_else(|| panic!("{handle}: {text}"));
                assert!(
                    !state.is_empty() && state.split(" · ").all(|part| items(part).is_none()),
                    "{handle}: the state after the numbers, {text}"
                );
            }
        } else {
            let login = written(&[order["logged_in"], order["known_user"]]);
            assert_eq!(line(&harness, "login"), format!("{login} · ✓ 2 checks"));
        }
    }

    // A failed check: the line red after its numbers, the border red.
    let source = shop_30().replace(
        r#"sample 201 { orderId: "o-1", status: "created", total: 172.5 }"#,
        r#"sample 201 { orderId: "o-1", status: "created", total: 180.0 }"#,
    );
    let (_project, mut harness) = open(&source, WIDE);
    simulate(&mut harness);
    let order = order(&graph(&harness));
    let checkout = written(&[
        order["checkout_total"],
        order["checkout_status"],
        order["checkout_created"],
    ]);
    show(&mut harness, "checkout", 0.6);
    let text = line(&harness, "checkout");
    assert!(
        text.starts_with(&format!("{checkout} · ✕ 1/3 failed")),
        "{text}"
    );
    let id = id_of(&harness, "checkout");
    let drawn = canvas(&harness).debug_texts(&id);
    let shown = drawn
        .iter()
        .find(|drawn| drawn.text == text)
        .unwrap_or_else(|| panic!("the line is written: {drawn:?}"));
    assert_eq!(shown.color, theme::DANGER, "red");
    assert_eq!(canvas(&harness).debug_signal(&id), Some(theme::DANGER));
}

/// Criterion 5: the Cleanup board numbers from 1, as its badges do.
#[test]
fn lay27_criterion_5_the_cleanup_board_numbers_from_one() {
    let (_project, mut harness) = open(&shop_30(), WIDE);
    harness.get_by_label("Cleanup").click();
    settle(&mut harness);
    harness.step();
    let cleanup = cleanup_graph(&harness);
    let removed = &cleanup
        .nodes
        .iter()
        .find(|node| node.handle == "removed")
        .expect("removed")
        .id;
    assert_eq!(
        canvas(&harness).debug_aggregate(removed).as_deref(),
        Some("2 · status == 204")
    );
    assert_eq!(
        canvas(&harness)
            .debug_badge(removed)
            .expect("removed has its badge")
            .text,
        "1"
    );
}

/// Criterion 6: a component call is one step with one number: `product` and
/// `cart` are 4 and 5, and the numbers on the board end at the number of
/// the scenario's steps.
#[test]
fn lay27_criterion_6_a_component_call_is_one_number() {
    let (_project, mut harness) = tidied("far-value", WIDE);
    fit(&mut harness);
    let board = canvas(&harness);
    let badge = |handle: &str| {
        board
            .debug_badge(&id_of(&harness, handle))
            .unwrap_or_else(|| panic!("{handle} has a badge"))
            .text
    };
    assert_eq!((badge("product"), badge("cart")), ("4".into(), "5".into()));
    let (badges, lines) = on_board(&harness);
    let last = badges.iter().chain(&lines).max().copied();
    assert_eq!(last, Some(sequence(&graph(&harness)).len()));
}

/// Criterion 7: below 45 %, at 960 × 540 after *Fit the view* and at
/// 1600 × 1000, a card whose pill has no room writes the numbers at the
/// right end of the name's row; every number of `far-value` is on the board
/// at 960 × 540; a pill reads the numbers before the words or alone; a card
/// that writes no name gets nothing.
#[test]
fn lay27_criterion_7_below_45_the_numbers_share_the_names_row() {
    let (_project, mut harness) = tidied("far-value", crate::testing::MINIMUM);
    fit(&mut harness);
    let seen = assert_numbers_below_45(&harness, "960 × 540 at Fit");
    assert!(
        seen.rows >= 4,
        "the numbers on the names' rows: {}",
        seen.rows
    );
    assert_every_number_once(&harness, "far-value at Fit, 960 × 540");

    let mut rows = 0;
    let mut pills = 0;
    let mut nameless = 0;
    for density in ["compact", "full"] {
        let (_project, mut harness) = tidied("far-value", WIDE);
        if density == "full" {
            switch_density(&mut harness);
            settle(&mut harness);
            harness.step();
        }
        for (handle, zoom) in [
            ("checkout", 0.44),
            ("checkout", 0.35),
            ("cart", 0.3),
            ("quantity", 0.25),
            ("checkout", 0.2),
        ] {
            show(&mut harness, handle, zoom);
            let seen = assert_numbers_below_45(
                &harness,
                &format!("{density}, 1600 × 1000, {handle} at {zoom}"),
            );
            rows += seen.rows;
            pills += seen.pills;
            nameless += seen.nameless;
        }
        // Further out a card holds no name, and gets no numbers.
        for zoom in [0.15, 0.12] {
            show(&mut harness, "checkout", zoom);
            for (id, _, texts) in in_view(&harness) {
                if !texts.iter().any(|text| text.kind == TextKind::Name) {
                    nameless += 1;
                    assert!(
                        texts.iter().all(|text| text.kind != TextKind::Result),
                        "{density} at {zoom}, {id}: no name, no numbers: {texts:?}"
                    );
                }
            }
        }
    }
    assert!(rows > 0 && pills > 0, "rows {rows}, pills {pills}");
    assert!(nameless > 0, "a card that writes no name was seen");

    // A failed check: its numbers red, in the pill or on the name's row.
    let source = shop_30().replace(
        r#"sample 201 { orderId: "o-1", status: "created", total: 172.5 }"#,
        r#"sample 201 { orderId: "o-1", status: "created", total: 180.0 }"#,
    );
    let (_project, mut harness) = open(&source, WIDE);
    simulate(&mut harness);
    show(&mut harness, "checkout", 0.35);
    let seen = assert_numbers_below_45(&harness, "shop-30 failed, 1600 × 1000 at 0.35");
    assert!(seen.rows + seen.pills > 0);
    assert!(
        canvas(&harness)
            .debug_texts(&id_of(&harness, "checkout"))
            .iter()
            .any(|text| text.kind == TextKind::Result && text.color == theme::DANGER),
        "checkout's numbers are red"
    );
}

/// Criterion 8: no text is smaller after zooming in across 0.45 and 0.75,
/// the numbers on both sides, and zooming moves no card.
#[test]
fn lay27_criterion_8_sizes_are_monotone_and_nothing_moves() {
    let (_project, mut harness) = tidied("far-value", WIDE);
    for density in ["compact", "full"] {
        if density == "full" {
            switch_density(&mut harness);
            settle(&mut harness);
            harness.step();
        }
        for (below, above) in [(0.44, 0.46), (0.74, 0.76)] {
            for handle in ["quantity", "checkout", "cart", "lookup", "view"] {
                let id = id_of(&harness, handle);
                show(&mut harness, handle, below);
                let before = canvas(&harness).debug_texts(&id);
                show(&mut harness, handle, above);
                let after = canvas(&harness).debug_texts(&id);
                for kind in [TextKind::Name, TextKind::Number, TextKind::Result] {
                    let smallest = |texts: &[DrawnText]| {
                        texts
                            .iter()
                            .filter(|text| text.kind == kind)
                            .map(|text| text.size)
                            .reduce(f32::min)
                    };
                    if let Some(small) = smallest(&before) {
                        let large = smallest(&after).unwrap_or_else(|| {
                            panic!("{density}: {handle}'s {kind:?} vanished at {above}")
                        });
                        assert!(
                            large >= small,
                            "{density}: {handle}'s {kind:?} shrank from {small} px at \
                             {below} to {large} px at {above}"
                        );
                    }
                }
                for (zoom, texts) in [(below, &before), (above, &after)] {
                    assert!(
                        texts.iter().any(|text| text.kind == TextKind::Result
                            && !numbers_in(&text.text).is_empty()),
                        "{density}: {handle} writes its numbers at {zoom}: {texts:?}"
                    );
                }
            }
        }
        show(&mut harness, "checkout", 0.2);
        let reference = world(&harness);
        for zoom in [0.25, 0.35, 0.44, 0.46, 0.6, 0.74, 0.76, 1.0] {
            show(&mut harness, "checkout", zoom);
            assert_eq!(world(&harness), reference, "{density} at {zoom}");
        }
    }
}

/// Criterion 9: with *Collapse checks* off, and for a step with *Expand
/// checks here*, the line is today's text and the checks carry their badges;
/// a gate, which other steps read, keeps card and badge and is in no line.
#[test]
fn lay27_criterion_9_expanded_checks_are_not_named() {
    let (_project, mut harness) = open(&shop_30(), WIDE);
    let order = order(&graph(&harness));
    show(&mut harness, "login", 0.6);
    switch_checks(&mut harness);
    show(&mut harness, "login", 0.6);
    assert_eq!(line(&harness, "login"), "2 checks");
    for check in ["logged_in", "known_user"] {
        let badge = canvas(&harness)
            .debug_badge(&id_of(&harness, check))
            .unwrap_or_else(|| panic!("{check} has its badge"));
        assert_eq!(badge.text, order[check].to_string());
    }
    show(&mut harness, "cart", 0.6);
    assert_eq!(line(&harness, "cart"), "3 checks");

    switch_checks(&mut harness);
    show(&mut harness, "checkout", 1.0);
    harness.get_by_label("Checks of checkout").click();
    settle(&mut harness);
    harness.step();
    harness.get_by_label("Expand checks here").click();
    settle(&mut harness);
    harness.step();
    show(&mut harness, "checkout", 0.6);
    assert_eq!(line(&harness, "checkout"), "3 checks");
    for check in ["checkout_total", "checkout_status", "checkout_created"] {
        let badge = canvas(&harness)
            .debug_badge(&id_of(&harness, check))
            .unwrap_or_else(|| panic!("{check} hangs under checkout with its badge"));
        assert_eq!(badge.text, order[check].to_string());
    }
    // The other steps keep their numbers.
    assert_eq!(
        line(&harness, "login"),
        format!(
            "{} · 2 checks",
            written(&[order["logged_in"], order["known_user"]])
        )
    );

    show(&mut harness, "valid_cart", 0.6);
    let gate = id_of(&harness, "valid_cart");
    assert!(!canvas(&harness).debug_hidden().contains(&gate));
    assert_eq!(
        canvas(&harness)
            .debug_badge(&gate)
            .expect("the gate has its badge")
            .text,
        order["valid_cart"].to_string()
    );
    for node in graph(&harness).nodes {
        if let Some(text) = canvas(&harness).debug_aggregate(&node.id) {
            assert!(
                !numbers_in(&text).contains(&order["valid_cart"]),
                "the gate is in no line: {}'s {text}",
                node.handle
            );
        }
    }
}

/// Criterion 10: a step added in code before step 5 renumbers the lines in
/// the next frame, with the badges.
#[test]
fn lay27_criterion_10_an_edit_renumbers_the_lines() {
    let source = shop_30();
    let (_project, mut harness) = open(&source, WIDE);
    show(&mut harness, "cart", 0.6);
    assert_eq!(line(&harness, "login"), "3–4 · 2 checks");
    assert_eq!(line(&harness, "cart"), "13–14, 27 · 3 checks");

    let anchor = "  catalogue = GET \"/products\" {";
    assert!(source.contains(anchor), "the fixture's catalogue");
    let edited = source.replacen(
        anchor,
        &format!("  probe = GET \"/probe\" {{\n    sample 200 {{}}\n  }}\n{anchor}"),
        1,
    );
    let document = document(&harness);
    app(&mut harness)
        .session
        .as_mut()
        .unwrap()
        .edit_source(&document, &edited, CODE)
        .unwrap();
    settle(&mut harness);
    harness.step();
    assert_eq!(line(&harness, "login"), "3–4 · 2 checks", "before it");
    assert_eq!(line(&harness, "cart"), "14–15, 28 · 3 checks");
    show(&mut harness, "cart", 0.6);
    assert_eq!(
        canvas(&harness)
            .debug_badge(&id_of(&harness, "cart"))
            .expect("cart's badge")
            .text,
        "13"
    );
    let order = order(&graph(&harness));
    assert_eq!(
        (
            order["cart_created"],
            order["cart_total"],
            order["receipt_total"]
        ),
        (14, 15, 28)
    );
}

/// Criterion 11, design review: `far-value` with checks collapsed at
/// 1600 × 1000 and 960 × 540 after *Fit the view* (56 % and 24 % since
/// LAY-03), before and after a simulation, and one full card; each frame has
/// a card in view.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay27() {
    let card_in_view = |harness: &FolderHarness, label: &str| {
        assert!(!in_view(harness).is_empty(), "{label}: a card is in view");
    };
    for (size, suffix) in [(WIDE, "1600"), (crate::testing::MINIMUM, "960")] {
        let (_project, mut harness) = tidied("far-value", size);
        fit(&mut harness);
        let zoom = canvas(&harness).debug_zoom();
        assert_eq!(
            zoom < 0.45,
            size == crate::testing::MINIMUM,
            "Fit at {zoom}"
        );
        card_in_view(&harness, "before");
        crate::testing::capture(&mut harness, &format!("lay27-before-{suffix}"), None);
        simulate(&mut harness);
        if size == crate::testing::MINIMUM {
            // Run results folded, so the board is as tall as before the run.
            harness.get_by_label("Run results").click();
            settle(&mut harness);
        }
        fit(&mut harness);
        let after = canvas(&harness).debug_zoom();
        assert!((after - zoom).abs() < 0.05, "Fit at {after} after the run");
        card_in_view(&harness, "after");
        crate::testing::capture(&mut harness, &format!("lay27-after-{suffix}"), None);
    }
    let (_project, mut harness) = tidied("far-value", WIDE);
    switch_density(&mut harness);
    settle(&mut harness);
    harness.step();
    simulate(&mut harness);
    show(&mut harness, "checkout", 1.0);
    card_in_view(&harness, "full card");
    crate::testing::capture(&mut harness, "lay27-full-card-1600", None);
}

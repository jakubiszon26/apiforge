//! LAY-45 (#468): the ordering lines of a chain read left to right, row by
//! row. Out of a step on its right at the title line, into the next on its
//! left at the title line; at the end of a row one line through the row gap
//! into the first step of the next row. In both *Wires* modes; the arrow at
//! the badge (LAY-24) and the mark at a row's end (LAY-06) go.
//!
//! Criterion 8 is three measures of crossings (the owner's answer A of
//! 2026-11-20 on #468): data lines against data lines no more than before,
//! the chain against a data line only at a right angle with the bridge on
//! the data line, and nothing within 20 px alongside (`packed` 0).
use super::lay28::{board, drawn, tidied};
use super::*;
use crate::canvas::WireMode;
use apiforge_core::{format::Graph, model::NodeKind};
use apiforge_layout::{GAP, Rows, crossing, groups, packed_outside_groups};
use egui::{Pos2, Rect, pos2, vec2};
use egui_kittest::kittest::Queryable;

const REFERENCE: [&str; 5] = ["order-flow", "shop-30", "shop-50", "fan-out", "labels"];
const FIT: &str = "Fit the view";
const TIDY: &str = "Tidy up";
const MODES: [WireMode; 2] = [WireMode::All, WireMode::Chips];

/// A board of the criteria: a reference scenario of `fixtures/scenarios/lay/`
/// or `booking-day` of the room project.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Scenario {
    Lay(&'static str),
    BookingDay,
}

/// The boards criteria 1 to 7 are measured on.
const BOARDS: [Scenario; 3] = [
    Scenario::Lay("labels"),
    Scenario::Lay("shop-30"),
    Scenario::BookingDay,
];

fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// The room project copied to a temporary directory, `path` open on its
/// board in the *Wires* mode given, its checks folded or drawn, tidied with
/// nothing selected.
fn rooms(
    path: &str,
    size: egui::Vec2,
    expanded: bool,
    wires: WireMode,
    prepare: impl FnOnce(&std::path::Path),
) -> (tempfile::TempDir, FolderHarness, String) {
    let project = tempfile::tempdir().unwrap();
    copy_tree(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/scenarios/lay/rooms"),
        project.path(),
    );
    prepare(project.path());
    let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).debug_wires(wires);
    if expanded {
        app(&mut harness).debug_draw_checks();
    }
    app(&mut harness).review_open(path, None);
    settle(&mut harness);
    let id = folder(&harness)
        .session
        .as_ref()
        .unwrap()
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == path)
        .unwrap_or_else(|| panic!("the room project has {path}"))
        .id()
        .to_owned();
    app(&mut harness).selections.insert(id.clone(), Vec::new());
    settle(&mut harness);
    choose(&mut harness, TIDY);
    settle(&mut harness);
    harness.step();
    (project, harness, id)
}

fn open(
    scenario: Scenario,
    wires: WireMode,
    expanded: bool,
) -> (tempfile::TempDir, FolderHarness, String) {
    match scenario {
        Scenario::Lay(name) => tidied(name, WIDE, expanded, wires),
        Scenario::BookingDay => rooms("scenarios/booking-day.afs", WIDE, expanded, wires, |_| {}),
    }
}

fn graph(harness: &FolderHarness, id: &str, cleanup: bool) -> Graph {
    let snapshot = folder(harness).snapshot(id).unwrap();
    graph_of(snapshot.model.as_ref().unwrap(), cleanup).clone()
}

fn handle_of(graph: &Graph, id: &str) -> String {
    graph
        .nodes
        .iter()
        .find(|node| node.id == id)
        .map_or_else(|| id.to_owned(), |node| node.handle.clone())
}

fn id_of(graph: &Graph, handle: &str) -> String {
    graph
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("a step {handle}"))
        .id
        .clone()
}

/// The lines of the chain the last frame drew.
fn chain(harness: &FolderHarness, id: &str) -> Vec<crate::canvas::DrawnWire> {
    board(harness, id)
        .debug_wires()
        .iter()
        .filter(|wire| wire.chain && !wire.points.is_empty())
        .cloned()
        .collect()
}

/// A card's box on the board.
fn world(harness: &FolderHarness, id: &str, card: &str) -> Rect {
    board(harness, id)
        .debug_world(card)
        .unwrap_or_else(|| panic!("{card} is drawn"))
}

/// A board point on screen, with the last frame's camera.
fn on_screen(canvas: &crate::canvas::Canvas, world: Pos2) -> Pos2 {
    let (pan, zoom) = canvas.debug_camera();
    canvas.debug_viewport().min + pan + world.to_vec2() * zoom
}

fn near(a: Pos2, b: Pos2) -> bool {
    (a - b).length() < 0.5
}

/// Asserts that every line of the chain leaves its source at the socket
/// on the right edge and enters its target at the socket on the left edge,
/// both on the card's title line, in its header row.
fn assert_sockets(harness: &FolderHarness, id: &str, context: &str) -> usize {
    let canvas = board(harness, id);
    let lines = chain(harness, id);
    for line in &lines {
        let source = line
            .source
            .as_deref()
            .expect("a line of the chain leaves a card");
        let (from, to) = (world(harness, id, source), world(harness, id, &line.target));
        let (_, out) = canvas.debug_order_sockets(source);
        let (into, _) = canvas.debug_order_sockets(&line.target);
        let (out, into) = (out.expect("a socket out"), into.expect("a socket in"));
        let (first, last) = (line.points[0], line.points[line.points.len() - 1]);
        let label = format!("{context}: {} -> {}", source, line.target);
        assert!(
            near(first, out),
            "{label} starts at {first:?}, the socket is at {out:?}"
        );
        assert!(
            near(last, into),
            "{label} ends at {last:?}, the socket is at {into:?}"
        );
        assert!(
            (out.x - from.right()).abs() < 0.5,
            "{label}: out on the right edge"
        );
        assert!(
            (into.x - to.left()).abs() < 0.5,
            "{label}: in on the left edge"
        );
        for (card, socket) in [(source, out), (line.target.as_str(), into)] {
            let header = canvas
                .debug_header(card)
                .unwrap_or_else(|| panic!("{label}: {card} has a header row"));
            let at = on_screen(canvas, socket).y;
            assert!(
                at > header.top() && at < header.bottom(),
                "{label}: the socket of {card} at {at} is on its title row {header:?}"
            );
        }
    }
    lines.len()
}

/// Criterion 1: every line of the chain starts at the source's socket on
/// the right edge and ends at the target's on the left edge, both on the
/// title line, in both *Wires* modes.
#[test]
fn lay45_criterion_1_out_on_the_right_in_on_the_left() {
    for scenario in BOARDS {
        for wires in MODES {
            let (_project, harness, id) = open(scenario, wires, false);
            let context = format!("{scenario:?}, {wires:?}");
            let lines = assert_sockets(&harness, &id, &context);
            assert!(lines >= 4, "{context}: the chain is drawn, {lines} lines");
        }
    }
}

/// The rows of a tidied board: the spine cards of each row, left to right,
/// a row being the cards whose tops are level.
fn rows(harness: &FolderHarness, id: &str, graph: &Graph) -> Vec<Vec<(String, Rect)>> {
    let mut rows: Vec<Vec<(String, Rect)>> = Vec::new();
    for node in graph.nodes.iter().filter(|node| {
        matches!(
            node.kind,
            NodeKind::Start | NodeKind::Request | NodeKind::Component | NodeKind::If
        )
    }) {
        let Some(rect) = board(harness, id).debug_world(&node.id) else {
            continue;
        };
        match rows
            .iter_mut()
            .find(|row| (row[0].1.top() - rect.top()).abs() < 0.5)
        {
            Some(row) => row.push((node.id.clone(), rect)),
            None => rows.push(vec![(node.id.clone(), rect)]),
        }
    }
    for row in &mut rows {
        row.sort_by(|a, b| a.1.left().total_cmp(&b.1.left()));
    }
    rows.sort_by(|a, b| a[0].1.top().total_cmp(&b[0].1.top()));
    rows
}

/// Whether the segment from `a` to `b` has a piece inside `rect`.
fn passes(a: Pos2, b: Pos2, rect: Rect) -> bool {
    let steps = (a.distance(b) * 2.0).ceil().max(1.0) as usize;
    (0..=steps).any(|step| rect.contains(a.lerp(b, step as f32 / steps as f32)))
}

/// Criterion 2: between two neighbours of a row the line is one straight
/// segment, and no line of the chain passes between two cards of a row but
/// the one that joins them.
#[test]
fn lay45_criterion_2_neighbours_are_joined_straight() {
    for scenario in BOARDS {
        for wires in MODES {
            let (_project, harness, id) = open(scenario, wires, false);
            let context = format!("{scenario:?}, {wires:?}");
            let graph = graph(&harness, &id, false);
            let lines = chain(&harness, &id);
            let mut straight = 0;
            for row in rows(&harness, &id, &graph) {
                for pair in row.windows(2) {
                    let ((left, a), (right, b)) = (&pair[0], &pair[1]);
                    let gap = Rect::from_min_max(
                        pos2(a.right() + 0.5, a.top().max(b.top()) + 0.5),
                        pos2(b.left() - 0.5, a.bottom().min(b.bottom()) - 0.5),
                    );
                    for line in &lines {
                        let joins =
                            line.source.as_deref() == Some(left.as_str()) && line.target == *right;
                        if joins {
                            straight += 1;
                            assert_eq!(
                                line.points.len(),
                                2,
                                "{context}: {} -> {} is one segment: {:?}",
                                handle_of(&graph, left),
                                handle_of(&graph, right),
                                line.points
                            );
                            assert!(
                                (line.points[0].y - line.points[1].y).abs() < 0.5,
                                "{context}: level"
                            );
                            continue;
                        }
                        assert!(
                            !line
                                .points
                                .windows(2)
                                .any(|segment| passes(segment[0], segment[1], gap)),
                            "{context}: the line {} -> {} passes between {} and {}",
                            handle_of(&graph, line.source.as_deref().unwrap_or_default()),
                            handle_of(&graph, &line.target),
                            handle_of(&graph, left),
                            handle_of(&graph, right)
                        );
                    }
                }
            }
            assert!(straight >= 3, "{context}: neighbours joined, {straight}");
        }
    }
}

/// Criterion 3: after Tidy up the line to the next row runs in the row
/// gap — below every card of the row it leaves, above every card of the
/// row it enters — and keeps 20 px from every card and line; on `labels`
/// with chips, where it is the only line in the gap, the row gap is the one
/// Tidy up leaves without it.
#[test]
fn lay45_criterion_3_the_line_to_the_next_row_runs_in_the_row_gap() {
    for scenario in BOARDS {
        for wires in MODES {
            for expanded in [false, true] {
                let (_project, harness, id) = open(scenario, wires, expanded);
                let context = format!("{scenario:?}, {wires:?}, checks drawn: {expanded}");
                let graph = graph(&harness, &id, false);
                let rows = rows(&harness, &id, &graph);
                let cards: Vec<Rect> = board(&harness, &id)
                    .debug_cards()
                    .iter()
                    .filter_map(|(card, _)| board(&harness, &id).debug_world(card))
                    .collect();
                let mut changes = 0;
                for pair in rows.windows(2) {
                    let (last, first) = (pair[0].last().unwrap(), &pair[1][0]);
                    // A row that starts in an `if`'s braces is entered by
                    // its branch, which keeps its look (criterion 4).
                    let Some(line) = chain(&harness, &id).into_iter().find(|line| {
                        line.source.as_deref() == Some(last.0.as_str()) && line.target == first.0
                    }) else {
                        assert!(
                            graph
                                .edges
                                .iter()
                                .any(|edge| { edge.target == first.0 && edge.branch.is_some() }),
                            "{context}: a line to the next row, into {}",
                            handle_of(&graph, &first.0)
                        );
                        continue;
                    };
                    changes += 1;
                    // Every card standing in the rows' band, hung ones too.
                    let above = cards
                        .iter()
                        .filter(|card| card.top() < first.1.top() - 0.5)
                        .map(|card| card.bottom())
                        .fold(f32::MIN, f32::max);
                    let below = first.1.top();
                    let runs = line.points.windows(2).any(|segment| {
                        (segment[0].y - segment[1].y).abs() < 0.5
                            && segment[0].y >= above + GAP - 0.5
                            && segment[0].y <= below - GAP + 0.5
                            && segment[0].x.max(segment[1].x) >= last.1.right()
                            && segment[0].x.min(segment[1].x) <= first.1.left()
                    });
                    assert!(
                        runs,
                        "{context}: the line {} -> {} runs in the row gap: {:?}",
                        handle_of(&graph, &last.0),
                        handle_of(&graph, &first.0),
                        line.points
                    );
                }
                assert!(changes >= 1, "{context}: the chain changes rows");
                let (routes, blocks) = drawn(&harness, &id);
                assert_eq!(
                    packed_outside_groups(&routes, &blocks, &groups(&blocks), GAP),
                    [],
                    "{context}: the gap holds"
                );
                // With the checks drawn every card is in the row's band; a
                // folded check keeps its place there unseen (LAY-11).
                if scenario == Scenario::Lay("labels") && wires == WireMode::Chips && expanded {
                    for pair in rows.windows(2) {
                        let bottom = cards
                            .iter()
                            .filter(|card| card.top() < pair[1][0].1.top() - 0.5)
                            .map(|card| card.bottom())
                            .fold(f32::MIN, f32::max);
                        let gap = pair[1][0].1.top() - bottom;
                        assert!(
                            gap <= Rows::default().row_gap + 0.5,
                            "{context}: the row gap is {gap}, no more than it needs"
                        );
                    }
                }
            }
        }
    }
}

/// Criterion 4: the lines out of *Yes* and *No* keep their sockets and
/// their look: they leave the `if` above its *Yes* or *No* socket (or at
/// it, into a card hung right under it) and are drawn dashed, as a branch.
#[test]
fn lay45_criterion_4_the_branches_keep_their_sockets() {
    for scenario in [Scenario::Lay("shop-30"), Scenario::BookingDay] {
        for wires in MODES {
            let (_project, harness, id) = open(scenario, wires, false);
            let context = format!("{scenario:?}, {wires:?}");
            let graph = graph(&harness, &id, false);
            let mut branches = 0;
            for wire in board(&harness, &id).debug_wires() {
                let Some(edge) = graph.edges.iter().find(|edge| edge.id == wire.id) else {
                    continue;
                };
                let Some(branch) = edge.branch else {
                    continue;
                };
                if wire.points.is_empty() {
                    continue;
                }
                branches += 1;
                assert!(wire.dashed && !wire.chain, "{context}: a branch is dashed");
                let source = world(&harness, &id, wire.source.as_deref().unwrap());
                let x = source.left()
                    + match branch {
                        apiforge_core::model::Branch::True => 70.0,
                        apiforge_core::model::Branch::False => 210.0,
                    };
                let first = wire.points[0];
                assert!(
                    (first.x - x).abs() < 0.5
                        && ((first.y - source.top()).abs() < 0.5
                            || (first.y - source.bottom()).abs() < 0.5),
                    "{context}: {branch:?} leaves at {first:?}, its socket is at x {x}"
                );
            }
            assert!(branches >= 2, "{context}: branches drawn");
        }
    }
}

/// Criterion 5: no arrow at a badge and no mark at a row's end, in either
/// mode: no text, no hover statement `after …` and no accessible name of
/// them is left on the board, and the badge stays at the card's corner.
#[test]
fn lay45_criterion_5_the_arrow_and_the_mark_are_gone() {
    for scenario in BOARDS {
        for wires in MODES {
            let (_project, mut harness, id) = open(scenario, wires, false);
            let context = format!("{scenario:?}, {wires:?}");
            choose(&mut harness, FIT);
            harness.step();
            let graph = graph(&harness, &id, false);
            for node in &graph.nodes {
                let canvas = board(&harness, &id);
                for text in canvas.debug_texts(&node.id) {
                    assert!(
                        !text.text.starts_with('↳'),
                        "{context}: a mark {:?}",
                        text.text
                    );
                }
                assert!(
                    harness
                        .query_by_label_contains(&format!("{} after ", node.handle))
                        .is_none(),
                    "{context}: an arrow names {}",
                    node.handle
                );
            }
            assert!(
                harness.query_by_label_contains("↳").is_none(),
                "{context}: a mark is named"
            );
            // Pointing where the arrow was, left of each badge, states nothing.
            for (card, _) in board(&harness, &id).debug_cards().to_vec() {
                let Some(badge) = board(&harness, &id).debug_badge(&card) else {
                    continue;
                };
                let rect = board(&harness, &id).debug_to_screen(world(&harness, &id, &card));
                assert!(
                    badge.rect.center().distance(rect.left_top()) < 24.0,
                    "{context}: the badge of {} stays at the corner",
                    handle_of(&graph, &card)
                );
                harness.hover_at(badge.rect.left_center() - vec2(14.0, 0.0));
                harness.step();
                harness.step();
                let said = board(&harness, &id).debug_statement();
                assert!(
                    !said
                        .as_deref()
                        .is_some_and(|text| text.starts_with("after ")),
                    "{context}: pointing left of a badge says {said:?}"
                );
            }
        }
    }
}

/// Criterion 6: pointing at a line of the chain marks it as pointing at a
/// data line does — it is lit and the box at the pointer says which step
/// follows which — and selecting a step lights the lines of the chain into
/// and out of it.
#[test]
fn lay45_criterion_6_the_chain_is_traced_like_a_data_line() {
    for wires in MODES {
        let (_project, mut harness, id) = open(Scenario::Lay("shop-30"), wires, false);
        let context = format!("{wires:?}");
        let graph = graph(&harness, &id, false);
        let line = chain(&harness, &id)
            .into_iter()
            .find(|line| {
                line.source.as_deref() == Some(id_of(&graph, "catalogue").as_str())
                    && line.target == id_of(&graph, "product")
            })
            .expect("catalogue -> product");
        choose(&mut harness, FIT);
        harness.step();
        let at = super::lay14::point_on(&harness, &id, &line.id);
        harness.hover_at(at);
        harness.step();
        harness.step();
        harness.step();
        let canvas = board(&harness, &id);
        let lit = canvas
            .debug_wires()
            .iter()
            .find(|wire| wire.id == line.id)
            .unwrap();
        assert!(lit.lit, "{context}: the pointed line is lit");
        assert_eq!(
            canvas.debug_statement().as_deref(),
            Some("catalogue → product"),
            "{context}: the box says what follows what"
        );
        harness.remove_cursor();
        harness.step();
        harness.step();
        app(&mut harness)
            .selections
            .insert(id.clone(), vec![id_of(&graph, "product")]);
        settle(&mut harness);
        harness.step();
        let canvas = board(&harness, &id);
        for (from, to) in [("catalogue", "product"), ("product", "promoted")] {
            let wire = canvas
                .debug_wires()
                .iter()
                .find(|wire| {
                    wire.chain
                        && wire.source.as_deref() == Some(id_of(&graph, from).as_str())
                        && wire.target == id_of(&graph, to)
                })
                .unwrap_or_else(|| panic!("{context}: {from} -> {to} drawn"));
            assert!(
                wire.lit && !wire.dimmed,
                "{context}: {from} -> {to} is lit with product selected"
            );
        }
    }
}

/// Criterion 7: a card moved by hand keeps its sockets. `catalogue` on
/// `shop-30` dragged 200 px down and 100 px left: its lines of the chain
/// still leave and enter at its sockets, go round the cards, and keep 20
/// px from them and from the other lines.
#[test]
fn lay45_criterion_7_a_card_moved_by_hand_keeps_its_sockets() {
    for wires in MODES {
        let (_project, mut harness, id) = open(Scenario::Lay("shop-30"), wires, false);
        let context = format!("{wires:?}");
        let graph = graph(&harness, &id, false);
        let card = id_of(&graph, "catalogue");
        let header = board(&harness, &id)
            .debug_card_rect(&card)
            .expect("catalogue is drawn");
        let from = header.center_top() + vec2(0.0, 12.0);
        let zoom = board(&harness, &id).debug_zoom();
        super::lay16::drag(&mut harness, from, from + vec2(-100.0, 200.0) * zoom);
        settle(&mut harness);
        harness.step();
        harness.step();
        let moved = world(&harness, &id, &card);
        assert!(
            moved.top() > header.top(),
            "{context}: catalogue moved down"
        );
        assert_sockets(&harness, &id, &context);
        let (routes, blocks) = drawn(&harness, &id);
        let (_, wires_given) = board(&harness, &id).debug_geometry().clone();
        assert_eq!(
            apiforge_layout::violations(&routes, &wires_given, &blocks),
            [],
            "{context}: no line through a card"
        );
        let mine: Vec<usize> = board(&harness, &id)
            .debug_wires()
            .iter()
            .filter(|wire| !wire.points.is_empty())
            .enumerate()
            .filter(|(_, wire)| {
                wire.chain && (wire.source.as_deref() == Some(card.as_str()) || wire.target == card)
            })
            .map(|(index, _)| index)
            .collect();
        assert_eq!(mine.len(), 2, "{context}: the chain into and out of it");
        let breaches = packed_outside_groups(&routes, &blocks, &groups(&blocks), GAP);
        let near: Vec<_> = breaches
            .iter()
            .filter(|breach| {
                mine.contains(&breach.route)
                    || matches!(breach.near, apiforge_layout::Near::Route(other) if mine.contains(&other))
            })
            .collect();
        assert!(near.is_empty(), "{context}: its lines keep 20 px: {near:?}");
    }
}

/// Criterion 9: the cleanup and a component's body draw their chains the
/// same way: `booking-day`'s cleanup, `released -> cancelled`, and a
/// component of two requests, `session -> check`.
#[test]
fn lay45_criterion_9_cleanup_and_component_bodies() {
    for wires in MODES {
        let (_project, mut harness, id) = open(Scenario::BookingDay, wires, false);
        harness.get_by_label("Cleanup").click();
        settle(&mut harness);
        harness.step();
        let cleanup = graph(&harness, &id, true);
        let lines = assert_sockets(&harness, &id, &format!("cleanup, {wires:?}"));
        assert!(lines >= 1, "{wires:?}: the cleanup draws its chain");
        assert!(
            chain(&harness, &id).iter().any(|line| {
                line.source.as_deref() == Some(id_of(&cleanup, "released").as_str())
                    && line.target == id_of(&cleanup, "cancelled")
            }),
            "{wires:?}: released -> cancelled"
        );

        let (_project, harness, id) = rooms("components/sign-in.afs", WIDE, false, wires, |root| {
            fs::write(
                    root.join("components/sign-in.afs"),
                    "component sign_in \"Sign in\" {\n  \
                     description \"Open a session on the room booking API and return its token.\"\n  \
                     @label(\"User\")\n  \
                     input user: string = \"tester\"\n  \
                     session = POST \"/sessions\" {\n    \
                     body { user: param.user, password: \"secret\" }\n    \
                     sample 201 { token: \"tok-1\" }\n  \
                     }\n  \
                     check = GET \"/sessions/{{session.body.token}}\" {\n    \
                     sample 200 { ok: true }\n  \
                     }\n  \
                     export token: string = session.body.token\n\
                     }\n",
                )
                .unwrap();
        });
        let body = graph(&harness, &id, false);
        let lines = assert_sockets(&harness, &id, &format!("component, {wires:?}"));
        assert!(lines >= 1, "{wires:?}: the component draws its chain");
        assert!(
            chain(&harness, &id).iter().any(|line| {
                line.source.as_deref() == Some(id_of(&body, "session").as_str())
                    && line.target == id_of(&body, "check")
            }),
            "{wires:?}: session -> check"
        );
    }
}

/// What the window drew before this ticket, on `main` after LANG-13
/// (6a4a3da), at 1600 × 1000: each reference board in each *Wires* mode
/// with its checks folded and drawn, its data lines' crossings with each
/// other, and the zoom Fit gives at 960 × 540 and at 1600 × 1000.
/// (board, wires, checks drawn, data against data, fit at 960, fit at 1600)
const BEFORE: [(&str, WireMode, bool, usize, f32, f32); 20] = [
    ("order-flow", WireMode::All, false, 7, 0.259, 0.596),
    ("order-flow", WireMode::All, true, 18, 0.254, 0.583),
    ("order-flow", WireMode::Chips, false, 2, 0.203, 0.480),
    ("order-flow", WireMode::Chips, true, 2, 0.201, 0.476),
    ("shop-30", WireMode::All, false, 54, 0.173, 0.438),
    ("shop-30", WireMode::All, true, 77, 0.162, 0.410),
    ("shop-30", WireMode::Chips, false, 0, 0.195, 0.490),
    ("shop-30", WireMode::Chips, true, 0, 0.179, 0.466),
    ("shop-50", WireMode::All, false, 136, 0.092, 0.232),
    ("shop-50", WireMode::All, true, 189, 0.086, 0.217),
    ("shop-50", WireMode::Chips, false, 0, 0.104, 0.271),
    ("shop-50", WireMode::Chips, true, 0, 0.097, 0.253),
    ("fan-out", WireMode::All, false, 32, 0.212, 0.538),
    ("fan-out", WireMode::All, true, 37, 0.209, 0.518),
    ("fan-out", WireMode::Chips, false, 0, 0.233, 0.549),
    ("fan-out", WireMode::Chips, true, 0, 0.189, 0.442),
    ("labels", WireMode::All, false, 0, 0.145, 0.367),
    ("labels", WireMode::All, true, 0, 0.130, 0.329),
    ("labels", WireMode::Chips, false, 0, 0.139, 0.360),
    ("labels", WireMode::Chips, true, 0, 0.124, 0.323),
];

fn before(name: &str, wires: WireMode, expanded: bool) -> (usize, f32, f32) {
    let &(_, _, _, data, narrow, wide) = BEFORE
        .iter()
        .find(|row| row.0 == name && row.1 == wires && row.2 == expanded)
        .expect("a figure from before");
    (data, narrow, wide)
}

/// The three measures of criterion 8 on a board as the last frame drew
/// it: data lines' crossings with each other, the crossings of the chain
/// with a data line, and whether every line keeps 20 px (`packed` 0).
/// Asserts the rules that hold for every crossing of the chain with a data
/// line: at a right angle, with the bridge on the data line.
pub(super) fn measures(harness: &FolderHarness, id: &str, label: &str) -> (usize, usize, bool) {
    let wires: Vec<crate::canvas::DrawnWire> = board(harness, id)
        .debug_wires()
        .iter()
        .filter(|wire| wire.points.len() >= 2)
        .cloned()
        .collect();
    let to_layout = |point: Pos2| apiforge_layout::Point::new(point.x, point.y);
    let (mut data, mut chained) = (0, 0);
    for (index, a) in wires.iter().enumerate() {
        for b in &wires[index + 1..] {
            let both_data = !a.dashed && !b.dashed;
            let (chain_line, data_line) = match (a.chain, b.chain) {
                (true, false) if !b.dashed => (a, b),
                (false, true) if !a.dashed => (b, a),
                _ if both_data => (a, b),
                _ => continue,
            };
            for p in a.points.windows(2) {
                for q in b.points.windows(2) {
                    let Some(at) = crossing(
                        to_layout(p[0]),
                        to_layout(p[1]),
                        to_layout(q[0]),
                        to_layout(q[1]),
                    ) else {
                        continue;
                    };
                    if both_data {
                        data += 1;
                        continue;
                    }
                    chained += 1;
                    let at = pos2(at.x, at.y);
                    let level = |s: &[Pos2]| (s[0].y - s[1].y).abs() < 0.01;
                    let upright = |s: &[Pos2]| (s[0].x - s[1].x).abs() < 0.01;
                    assert!(
                        (level(p) && upright(q)) || (upright(p) && level(q)),
                        "{label}: the chain crosses {} at a right angle at {at:?}",
                        data_line.id
                    );
                    assert!(
                        data_line.bridged_at.iter().any(|bridge| near(*bridge, at))
                            && !chain_line.bridged_at.iter().any(|bridge| near(*bridge, at)),
                        "{label}: the bridge at {at:?} is on the data line {}",
                        data_line.id
                    );
                }
            }
        }
    }
    let (routes, blocks) = drawn(harness, id);
    let kept = packed_outside_groups(&routes, &blocks, &groups(&blocks), GAP).is_empty();
    (data, chained, kept)
}

/// Criterion 8: on every reference board, in both modes and both check
/// states, after Tidy up: data lines cross each other no more than before
/// this ticket; the chain crosses a data line only at a right angle, with
/// the bridge on the data line, counted and printed (the pull request's
/// table), not limited; and every line keeps 20 px (`packed` 0).
#[test]
fn lay45_criterion_8_three_measures_of_crossings() {
    for name in REFERENCE {
        for wires in MODES {
            for expanded in [false, true] {
                let (_project, harness, id) = tidied(name, WIDE, expanded, wires);
                let label = format!("{name}, {wires:?}, checks drawn: {expanded}");
                let (data, chained, kept) = measures(&harness, &id, &label);
                errln!("LAY-45 {label}: data×data {data}, chain×data {chained}, gap {kept}");
                let (most, _, _) = before(name, wires, expanded);
                if !HELD.contains(&(name, wires, expanded)) {
                    assert!(
                        data <= most,
                        "{label}: {data} data crossings, {most} before"
                    );
                }
                assert!(kept, "{label}: a line within 20 px of another or a card");
            }
        }
    }
}

/// The two boards whose data lines cross each other more than before: on
/// `main` the rails above the rows, routed first, steered them (`fan-out`
/// with every wire and its checks drawn: 37, and 46 on the same board with
/// no ordering line at all; `shop-50` with every wire, checks folded: 136,
/// and 142). The owner's answer A on #468 (2026-11-20): their data lines
/// are held to the same board without ordering lines, as every board's
/// are (`lay45_criterion_8_data_lines_against_the_board_without_order`).
const HELD: [(&str, WireMode, bool); 2] = [
    ("fan-out", WireMode::All, true),
    ("shop-50", WireMode::All, false),
];

/// The crossings of the data lines with each other on the board as the
/// router is given it, with every ordering line taken out: the lines of the
/// chain and the branches, which leave their `if` from the top or bottom.
fn without_order(harness: &FolderHarness, id: &str) -> usize {
    let (rectangles, wires) = board(harness, id).debug_geometry().clone();
    let data: Vec<apiforge_layout::Wire> = wires
        .into_iter()
        .filter(|wire| {
            !wire.chain
                && !matches!(
                    wire.from.side,
                    apiforge_layout::Side::Top | apiforge_layout::Side::Bottom
                )
        })
        .collect();
    let routes = apiforge_layout::route_grouped(
        &rectangles,
        &groups(&rectangles),
        &data,
        &apiforge_layout::Routing::default(),
        &[],
    );
    let mut count = 0;
    for (index, a) in routes.iter().enumerate() {
        for b in &routes[index + 1..] {
            for (a1, a2) in a.segments() {
                for (b1, b2) in b.segments() {
                    count += usize::from(crossing(a1, a2, b1, b2).is_some());
                }
            }
        }
    }
    count
}

/// Criterion 8, its first measure as the owner answered it on #468 (A,
/// 2026-11-20): the data lines cross each other no more than on the same
/// board routed without its ordering lines. On the two boards of [`HELD`]
/// that is their limit (`shop-50` with every wire, checks folded: 140
/// against 142; `fan-out` with every wire, checks drawn: 46 against 46);
/// every other board keeps to it or to its figure before this ticket,
/// which `lay45_criterion_8_three_measures_of_crossings` holds (`order-flow`
/// with chips, checks folded: 1, 0 without ordering lines, 2 before: the
/// chain takes the row gap the two lines out of `order` ran in, and below
/// the row they cannot pass each other).
#[test]
fn lay45_criterion_8_data_lines_against_the_board_without_order() {
    for name in REFERENCE {
        for wires in MODES {
            for expanded in [false, true] {
                let (_project, harness, id) = tidied(name, WIDE, expanded, wires);
                let label = format!("{name}, {wires:?}, checks drawn: {expanded}");
                let (data, _, _) = measures(&harness, &id, &label);
                let alone = without_order(&harness, &id);
                let (was, _, _) = before(name, wires, expanded);
                errln!(
                    "LAY-45 {label}: data×data {data}, without ordering lines {alone}, before {was}"
                );
                if HELD.contains(&(name, wires, expanded)) {
                    assert!(
                        data <= alone,
                        "{label}: {data} data crossings, {alone} without ordering lines"
                    );
                } else {
                    assert!(
                        data <= alone || data <= was,
                        "{label}: {data} data crossings, {alone} without ordering lines, \
                         {was} before"
                    );
                }
            }
        }
    }
}

/// Criterion 10: at 960 × 540 and 1600 × 1000 every reference board fits
/// at a zoom at most 0.02 below the one before, in both modes and both
/// check states.
#[test]
fn lay45_criterion_10_the_boards_fit_as_before() {
    for name in REFERENCE {
        for wires in MODES {
            for expanded in [false, true] {
                let label = format!("{name}, {wires:?}, checks drawn: {expanded}");
                let (_, narrow_before, wide_before) = before(name, wires, expanded);
                for (size, was) in [
                    (crate::testing::MINIMUM, narrow_before),
                    (WIDE, wide_before),
                ] {
                    let (_project, mut harness, id) = tidied(name, size, expanded, wires);
                    choose(&mut harness, FIT);
                    harness.step();
                    let zoom = board(&harness, &id).debug_zoom();
                    assert!(
                        zoom >= was - 0.02,
                        "{label} at {size:?}: Fit at {zoom:.3}, {was:.3} before"
                    );
                }
            }
        }
    }
}

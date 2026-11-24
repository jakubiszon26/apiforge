//! DIAG-06c (#490) at the window: the board shows and takes the chain. An
//! `if` has its own *Next*; the join lines after an `if` are gone and the
//! end of a branch names the step after it; a wire carried from a *Next*
//! with a successor fades that line and says what will follow what; a step
//! carried from the Library lands on the nearest line; the cards after an
//! add or a move stand where *Tidy up* would put them; each kind of card
//! has its sockets of the order; a selected line of the chain says which
//! step follows which. On `labels` and `rooms/booking-day`.
use super::diag06a::{graph, id_of, passes, rooms, show, steps};
use super::diag07::{board, empty_point, line_of};
use super::lay16::{move_to, press, release};
use super::*;
use crate::canvas::DropMark;
use crate::dnd::Payload;
use crate::testing::MINIMUM;
use apiforge_core::{block::Block, format::Graph, model::NodeKind};
use egui::{Pos2, Rect, pos2};

const SCENARIO: &str = "scenarios/booking-day.afs";

/// `notice` and its check `sent`, written before the `if` in the fixture.
const NOTICE: &str = "  notice = POST \"/notifications\" {\n    headers { Authorization: \"Bearer {{token}}\" }\n    body { channel: \"email\", text: \"Your booking is confirmed.\" }\n    sample 202 { id: \"n-1\" }\n  }\n  sent = assert notice.status == 202\n";

/// `booking-day` with `notice` written after the `if`, so that the `if`
/// has a step after it.
fn after_the_if(root: &std::path::Path) {
    let path = root.join(SCENARIO);
    let text = fs::read_to_string(&path).unwrap();
    assert_eq!(text.matches(NOTICE).count(), 1);
    let close = "    not_booked = assert first.status == 201\n  }\n";
    assert_eq!(text.matches(close).count(), 1);
    let text = text
        .replace(NOTICE, "")
        .replace(close, &format!("{close}{NOTICE}"));
    fs::write(path, text).unwrap();
}

fn booking_day() -> (tempfile::TempDir, FolderHarness, String) {
    rooms(SCENARIO, after_the_if)
}

fn screen(harness: &FolderHarness, id: &str, world: Pos2) -> Pos2 {
    board(harness, id)
        .debug_to_screen(Rect::from_min_max(world, world))
        .min
}

/// The output socket `port` of `card`, on screen.
fn out_socket(harness: &FolderHarness, id: &str, card: &str, port: &str) -> Pos2 {
    let card_id = id_of(&graph(harness, id), card);
    let (_, at) = board(harness, id)
        .debug_out_sockets(&card_id)
        .into_iter()
        .find(|(name, _)| name == port)
        .unwrap_or_else(|| panic!("{card} has a socket {port}"));
    screen(harness, id, at)
}

/// The order socket into `card`, on screen.
fn in_socket(harness: &FolderHarness, id: &str, card: &str) -> Pos2 {
    let card_id = id_of(&graph(harness, id), card);
    let (into, _) = board(harness, id).debug_order_sockets(&card_id);
    screen(harness, id, into.expect("an order socket in"))
}

/// Carries a wire from `port` of `source` to `to` and holds it there.
fn carry(harness: &mut FolderHarness, id: &str, source: &str, port: &str, to: Pos2) {
    let from = out_socket(harness, id, source, port);
    press(harness, from);
    move_to(harness, from, to);
    harness.step();
}

fn caption(harness: &FolderHarness, id: &str) -> Option<String> {
    board(harness, id).debug_caption().map(str::to_owned)
}

/// The line of the chain from `source` to `target` the last frame drew.
fn chain_line(
    harness: &FolderHarness,
    id: &str,
    source: &str,
    target: &str,
) -> Option<crate::canvas::DrawnWire> {
    let board_graph = graph(harness, id);
    let (source, target) = (id_of(&board_graph, source), id_of(&board_graph, target));
    board(harness, id)
        .debug_wires()
        .iter()
        .find(|wire| {
            wire.chain && wire.source.as_deref() == Some(source.as_str()) && wire.target == target
        })
        .cloned()
}

fn position(order: &[String], handle: &str) -> usize {
    super::diag06a::at(order, handle)
}

// Criterion 1 ----------------------------------------------------------------

#[test]
fn diag06c_criterion_1_next_of_an_if_opens_add_step() {
    let (_project, mut harness, id) = booking_day();
    show(&mut harness, &id, &["booked"]);
    let to = empty_point(&harness, &id);
    carry(&mut harness, &id, "booked", "__order", to);
    release(&mut harness, to);
    harness.step();
    assert!(board(&harness, &id).debug_menu_open(), "the menu opens");
    harness
        .get_all_by_label("HTTP request")
        .min_by(|a, b| {
            let distance = |rect: Rect| rect.center().distance(to);
            distance(a.rect()).total_cmp(&distance(b.rect()))
        })
        .expect("the menu offers a request")
        .click();
    settle(&mut harness);
    harness.step();
    let text = passes(&harness, &id);
    let order = steps(&harness, &id);
    let new = order
        .iter()
        .find(|step| step.starts_with("request"))
        .expect("a new request")
        .clone();
    assert_eq!(
        position(&order, &new),
        position(&order, "calendar") + 1,
        "{order:?}"
    );
    assert_eq!(
        position(&order, "notice"),
        position(&order, &new) + 1,
        "{order:?}"
    );
    let written = line_of(&text, &format!("{new} = "));
    assert!(
        text.lines()
            .nth(written)
            .unwrap()
            .starts_with(&format!("  {new} = ")),
        "after the whole if, in the main flow: {text}"
    );
}

#[test]
fn diag06c_criterion_1_next_of_an_if_moves_a_step() {
    let (_project, mut harness, id) = booking_day();
    show(&mut harness, &id, &["booked", "hold"]);
    let to = in_socket(&harness, &id, "hold");
    carry(&mut harness, &id, "booked", "__order", to);
    release(&mut harness, to);
    harness.step();
    let text = passes(&harness, &id);
    let order = steps(&harness, &id);
    assert_eq!(
        position(&order, "hold"),
        position(&order, "calendar") + 1,
        "{order:?}"
    );
    assert_eq!(
        position(&order, "notice"),
        position(&order, "hold") + 1,
        "{order:?}"
    );
    assert!(
        line_of(&text, "not_booked = ") < line_of(&text, "hold = "),
        "{text}"
    );
}

// Criterion 2 ----------------------------------------------------------------

#[test]
fn diag06c_criterion_2_no_join_lines_and_the_end_names_the_next_step() {
    let (_project, mut harness, id) = booking_day();
    show(&mut harness, &id, &["booked", "notice"]);
    let board_graph = graph(&harness, &id);
    let notice = id_of(&board_graph, "notice");
    let booked = id_of(&board_graph, "booked");
    let into_notice: Vec<_> = board(&harness, &id)
        .debug_wires()
        .iter()
        .filter(|wire| wire.dashed && wire.target == notice)
        .map(|wire| wire.source.clone())
        .collect();
    assert_eq!(
        into_notice,
        [Some(booked)],
        "only the if's own Next leads into the step after it"
    );
    assert!(
        harness.query_by_label("↳ notice").is_some(),
        "the last card of Yes names the next step"
    );
}

#[test]
fn diag06c_criterion_2_an_if_that_is_last_shows_none() {
    let (_project, harness, _id) = rooms(SCENARIO, |_| {});
    assert!(harness.query_by_label_contains("↳").is_none());
}

// Criterion 3 ----------------------------------------------------------------

#[test]
fn diag06c_criterion_3_over_the_board() {
    let (_project, mut harness, id) = booking_day();
    show(&mut harness, &id, &["first", "hold"]);
    let to = empty_point(&harness, &id);
    carry(&mut harness, &id, "first", "__order", to);
    let text = "New step after `first`, before `hold`";
    assert_eq!(caption(&harness, &id).as_deref(), Some(text));
    assert!(
        harness.query_by_label(text).is_some(),
        "in the accessibility tree"
    );
    let line = chain_line(&harness, &id, "first", "hold").expect("the line first → hold");
    assert!(line.faded, "the line it replaces fades");
    release(&mut harness, to);
    assert!(board(&harness, &id).debug_menu_open(), "today's menu opens");
}

#[test]
fn diag06c_criterion_3_over_a_step() {
    let (_project, mut harness, id) = booking_day();
    show(&mut harness, &id, &["first", "invite"]);
    let to = in_socket(&harness, &id, "invite");
    carry(&mut harness, &id, "first", "__order", to);
    let text = "Move `invite` after `first`";
    assert_eq!(caption(&harness, &id).as_deref(), Some(text));
    assert!(harness.query_by_label(text).is_some());
    release(&mut harness, to);
}

#[test]
fn diag06c_criterion_3_over_an_if() {
    let (_project, mut harness, id) = booking_day();
    show(&mut harness, &id, &["first", "booked"]);
    let to = in_socket(&harness, &id, "booked");
    carry(&mut harness, &id, "first", "__order", to);
    assert_eq!(
        caption(&harness, &id).as_deref(),
        Some("Move `booked` after `first` and the 2 steps inside it")
    );
    release(&mut harness, to);
}

#[test]
fn diag06c_criterion_3_over_a_step_of_other_braces() {
    let (_project, mut harness, id) = booking_day();
    show(&mut harness, &id, &["second", "notice"]);
    let to = in_socket(&harness, &id, "notice");
    carry(&mut harness, &id, "second", "__order", to);
    assert_eq!(
        caption(&harness, &id).as_deref(),
        Some("Move `notice` after `second` into Yes of `booked`")
    );
    release(&mut harness, to);
}

// Criterion 4 ----------------------------------------------------------------

/// Carries `block` from the Library to `to`, holds it, and returns the mark.
fn carry_block(harness: &mut FolderHarness, id: &str, block: Block, to: Pos2) -> Option<DropMark> {
    egui::DragAndDrop::set_payload(
        &harness.ctx,
        Payload::Node {
            block,
            component: None,
        },
    );
    let start = board(harness, id).debug_board().center();
    harness.hover_at(start);
    harness.step();
    move_to(harness, start, to);
    harness.step();
    board(harness, id).debug_drop_mark().cloned()
}

/// A point on screen on the drawn line, clear of the cards and of every
/// other line.
fn near_line(harness: &FolderHarness, id: &str, source: &str, target: &str) -> Pos2 {
    let line = chain_line(harness, id, source, target).expect("the line is drawn");
    super::lay14::point_on(harness, id, &line.id)
}

/// A point of the board in view more than 80 px from every line of the
/// order and clear of the cards.
fn far_point(harness: &FolderHarness, id: &str) -> Pos2 {
    let canvas = board(harness, id);
    let inside = canvas.debug_board().shrink(20.0);
    let lines: Vec<Vec<Pos2>> = canvas
        .debug_wires()
        .iter()
        .filter(|wire| wire.dashed)
        .map(|wire| {
            wire.points
                .iter()
                .map(|at| screen(harness, id, *at))
                .collect()
        })
        .collect();
    let cards: Vec<Rect> = canvas.debug_cards().iter().map(|(_, rect)| *rect).collect();
    let far = |at: Pos2| {
        lines.iter().all(|points| {
            points.windows(2).all(|pair| {
                let along = pair[1] - pair[0];
                let t = if along.length_sq() <= f32::EPSILON {
                    0.0
                } else {
                    ((at - pair[0]).dot(along) / along.length_sq()).clamp(0.0, 1.0)
                };
                (pair[0] + along * t).distance(at) > 80.0
            })
        }) && cards.iter().all(|rect| !rect.expand(20.0).contains(at))
    };
    let mut y = inside.top();
    while y < inside.bottom() {
        let mut x = inside.left();
        while x < inside.right() {
            if far(pos2(x, y)) {
                return pos2(x, y);
            }
            x += 10.0;
        }
        y += 10.0;
    }
    panic!("the board has a place far from every line");
}

fn diag06c_criterion_4_near_a_line_at(size: egui::Vec2) {
    let (_project, mut harness, id) = rooms(SCENARIO, after_the_if);
    harness.set_size(size);
    settle(&mut harness);
    show(&mut harness, &id, &["first", "hold"]);
    let before = graph(&harness, &id);
    let to = near_line(&harness, &id, "first", "hold");
    let mark = carry_block(&mut harness, &id, Block::Request, to);
    assert_eq!(
        mark,
        Some(DropMark::Line {
            source: id_of(&before, "first"),
            port: "__order".into(),
        }),
        "{size:?}: the mark is on the line first → hold"
    );
    release(&mut harness, to);
    settle(&mut harness);
    harness.step();
    passes(&harness, &id);
    let order = steps(&harness, &id);
    let new = position(&order, "first") + 1;
    assert!(order[new].starts_with("request"), "{size:?}: {order:?}");
    assert_eq!(order[new + 1], "hold", "{size:?}: {order:?}");
}

#[test]
fn diag06c_criterion_4_near_a_line() {
    diag06c_criterion_4_near_a_line_at(MINIMUM);
    diag06c_criterion_4_near_a_line_at(WIDE);
}

#[test]
fn diag06c_criterion_4_near_a_line_in_a_branch() {
    let (_project, mut harness, id) = booking_day();
    show(&mut harness, &id, &["second", "calendar"]);
    let to = near_line(&harness, &id, "second", "calendar");
    carry_block(&mut harness, &id, Block::Request, to);
    release(&mut harness, to);
    settle(&mut harness);
    harness.step();
    let text = passes(&harness, &id);
    let order = steps(&harness, &id);
    let new = order[position(&order, "second") + 1].clone();
    assert!(new.starts_with("request"), "{order:?}");
    let (at, line) = (
        line_of(&text, &format!("{new} = ")),
        text.lines()
            .nth(line_of(&text, &format!("{new} = ")))
            .unwrap(),
    );
    assert!(line.starts_with("    "), "in the braces: {text}");
    assert!(at < line_of(&text, "calendar = "), "{text}");
}

#[test]
fn diag06c_criterion_4_far_from_every_line() {
    let (_project, mut harness, id) = booking_day();
    let before = graph(&harness, &id);
    let to = far_point(&harness, &id);
    let mark = carry_block(&mut harness, &id, Block::Request, to);
    assert_eq!(
        mark,
        Some(DropMark::End {
            last: id_of(&before, "notice"),
        }),
        "the mark is behind the last step of the main flow"
    );
    release(&mut harness, to);
    settle(&mut harness);
    harness.step();
    passes(&harness, &id);
    let order = steps(&harness, &id);
    assert!(order.last().unwrap().starts_with("request"), "{order:?}");
}

#[test]
fn diag06c_criterion_4_a_check_lands_as_today() {
    let (_project, mut harness, id) = booking_day();
    show(&mut harness, &id, &["first", "hold"]);
    let to = near_line(&harness, &id, "first", "hold");
    let mark = carry_block(&mut harness, &id, Block::Comparison, to);
    assert_eq!(mark, None, "a check is not placed by a line");
    let before = graph(&harness, &id).nodes.len();
    release(&mut harness, to);
    settle(&mut harness);
    harness.step();
    let after = graph(&harness, &id);
    assert_eq!(after.nodes.len(), before + 1);
    assert_eq!(
        after.nodes.last().unwrap().kind,
        NodeKind::Assert,
        "written last"
    );
}

// Criterion 5 ----------------------------------------------------------------

/// The cards the room-making moves, by the steps after the place: those
/// steps from `from` to the end of the main flow, everything in the braces
/// of an `if` among them, and the cards hung under them in shape D.
fn after_the_place(board_graph: &Graph, from: &str) -> Vec<String> {
    let order = crate::canvas::shape::plan(board_graph, None).expect("a spine");
    let spine: Vec<&str> = order
        .spine
        .iter()
        .map(|&index| board_graph.nodes[index].id.as_str())
        .collect();
    let braces = apiforge_core::chain::braces(board_graph);
    let top: Vec<&str> = spine
        .iter()
        .copied()
        .filter(|id| !braces.contains_key(id))
        .collect();
    let start = top
        .iter()
        .position(|id| *id == from)
        .expect("a step of the main flow");
    let mut moved: Vec<String> = top[start..].iter().map(|id| (*id).to_owned()).collect();
    let inside = |id: &str, x: &str| {
        let mut current = id;
        while let Some(&(parent, _)) = braces.get(current) {
            if parent == x {
                return true;
            }
            current = parent;
        }
        false
    };
    for node in &board_graph.nodes {
        if top[start..].iter().any(|x| inside(&node.id, x)) {
            moved.push(node.id.clone());
        }
    }
    for &(satellite, under) in &order.satellites {
        if moved.iter().any(|id| id == spine[under]) {
            moved.push(board_graph.nodes[satellite].id.clone());
        }
    }
    moved.sort();
    moved.dedup();
    moved
}

/// After `edit`, the cards after the place stand as a whole Tidy up
/// arranges them, measured from the step they follow (DIAG-23), and every
/// other card keeps its position; the camera does not move. `from` names
/// the first step whose place changed, on the board after the edit. A card
/// is moved by hand before the place (`early`) and after it (`late`) first.
fn makes_room(
    view: &[&str],
    edit: impl FnOnce(&mut FolderHarness, &str),
    from: impl Fn(&Graph) -> String,
    early: &str,
    late: &str,
) {
    let (_project, mut harness, id) = booking_day();
    for (card, by) in [(early, 37.0), (late, -41.0)] {
        let moved = graph(&harness, &id)
            .nodes
            .iter()
            .find(|node| node.handle == card)
            .map(|node| {
                (
                    node.id.clone(),
                    apiforge_core::format::Position {
                        x: node.position.x + by,
                        y: node.position.y + by,
                    },
                )
            })
            .unwrap();
        app(&mut harness).active_board_command(crate::canvas::Command::Move(vec![moved]));
        settle(&mut harness);
    }
    // The cards a gesture is made between in view first; the whole board
    // for an add, whose new card the board keeps in view (UX-18).
    if view.is_empty() {
        choose(&mut harness, "Fit the view");
        settle(&mut harness);
        harness.step();
    } else {
        show(&mut harness, &id, view);
    }
    let before = graph(&harness, &id);
    let camera = board(&harness, &id).debug_camera();
    edit(&mut harness, &id);
    settle(&mut harness);
    harness.step();
    passes(&harness, &id);
    let after = graph(&harness, &id);
    let tidied = folder(&harness).debug_tidied(&id).expect("a tidied board");
    let first = from(&after);
    let moved = after_the_place(&after, &first);
    // The step before the place in its braces.
    let follows = {
        let written = apiforge_core::chain::braces(&after);
        let own: Vec<&str> = after
            .nodes
            .iter()
            .filter(|node| apiforge_core::chain::is_step(node.kind))
            .map(|node| node.id.as_str())
            .filter(|id| written.get(id) == written.get(first.as_str()))
            .collect();
        let at = own.iter().position(|id| *id == first).expect("a step");
        own[at - 1].to_owned()
    };
    let stands = |graph: &Graph, id: &str| {
        let node = graph.nodes.iter().find(|node| node.id == id).unwrap();
        (node.position.x, node.position.y)
    };
    let (at, tidy_at) = (stands(&after, &follows), stands(&tidied, &follows));
    let offset = (at.0 - tidy_at.0, at.1 - tidy_at.1);
    let early_id = id_of(&after, early);
    assert!(
        !moved.contains(&early_id),
        "`{early}` stands before the place"
    );
    assert!(
        moved.contains(&id_of(&after, late)),
        "`{late}` stands after the place"
    );
    for node in &after.nodes {
        if moved.contains(&node.id) {
            let tidy = stands(&tidied, &node.id);
            assert_eq!(
                (node.position.x, node.position.y),
                (tidy.0 + offset.0, tidy.1 + offset.1),
                "`{}` stands as Tidy up arranges it",
                node.handle
            );
        } else if let Some(old) = before.nodes.iter().find(|n| n.id == node.id) {
            assert_eq!(
                node.position, old.position,
                "`{}` keeps its place",
                node.handle
            );
        }
    }
    assert_eq!(
        board(&harness, &id).debug_camera(),
        camera,
        "the camera does not move"
    );
}

fn added(after: &Graph) -> String {
    after
        .nodes
        .iter()
        .find(|node| node.handle.starts_with("request"))
        .expect("the new step")
        .id
        .clone()
}

#[test]
fn diag06c_criterion_5_an_add() {
    makes_room(
        &[],
        |harness, id| {
            let first = id_of(&graph(harness, id), "first");
            app(harness).active_board_command(crate::canvas::Command::AddNode {
                block: Block::Request,
                component: None,
                position: apiforge_core::format::Position { x: 0.0, y: 0.0 },
                source: Some((first, "__order".into())),
            });
        },
        added,
        "rooms",
        "invite",
    );
}

/// Moves `step` after `anchor` by a wire, as the board does.
fn wire_move(harness: &mut FolderHarness, id: &str, anchor: &str, step: &str) {
    super::diag06a::wire(harness, id, anchor, "__order", step);
}

#[test]
fn diag06c_criterion_5_a_move_later() {
    makes_room(
        &["invite", "hold"],
        |harness, id| wire_move(harness, id, "invite", "hold"),
        |after| id_of(after, "invite"),
        "rooms",
        "booked",
    );
}

#[test]
fn diag06c_criterion_5_a_move_earlier() {
    makes_room(
        &["first", "invite"],
        |harness, id| wire_move(harness, id, "first", "invite"),
        |after| id_of(after, "invite"),
        "rooms",
        "hold",
    );
}

#[test]
fn diag06c_criterion_5_an_if_moved() {
    makes_room(
        &["first", "booked"],
        |harness, id| wire_move(harness, id, "first", "booked"),
        |after| id_of(after, "booked"),
        "rooms",
        "hold",
    );
}

// Criterion 6 ----------------------------------------------------------------

/// The order sockets of `card`: in, and the ports out.
fn sockets(harness: &FolderHarness, id: &str, card: &str) -> (bool, Vec<String>) {
    let card_id = id_of(&graph(harness, id), card);
    let canvas = board(harness, id);
    let (into, _) = canvas.debug_order_sockets(&card_id);
    let mut out: Vec<String> = canvas
        .debug_out_sockets(&card_id)
        .into_iter()
        .map(|(port, _)| port)
        .collect();
    out.sort();
    (into.is_some(), out)
}

#[test]
fn diag06c_criterion_6_sockets_by_kind() {
    let (_project, mut harness, id) = booking_day();
    app(&mut harness).debug_draw_checks();
    settle(&mut harness);
    harness.step();
    let cases: [(&str, bool, &[&str]); 6] = [
        ("begin", false, &["__order"]),
        ("first", true, &["__order"]),
        ("token", true, &["__order"]),
        ("booked", true, &["__false", "__order", "__true"]),
        ("sent", true, &[]),
        ("slot", true, &[]),
    ];
    for (card, into, out) in cases {
        assert_eq!(
            sockets(&harness, &id, card),
            (into, out.iter().map(|port| (*port).to_owned()).collect()),
            "`{card}`"
        );
    }
}

#[test]
fn diag06c_criterion_6_an_export() {
    let (_project, harness, id) = rooms("components/sign-in.afs", |_| {});
    assert_eq!(sockets(&harness, &id, "token"), (true, vec![]));
    assert_eq!(
        sockets(&harness, &id, "session"),
        (true, vec!["__order".to_owned()])
    );
}

// Criterion 7 ----------------------------------------------------------------

#[test]
fn diag06c_criterion_7() {
    let (_project, mut harness, id) = booking_day();
    show(&mut harness, &id, &["first", "hold"]);
    let line = chain_line(&harness, &id, "first", "hold").expect("the line first → hold");
    let at = super::lay14::point_on(&harness, &id, &line.id);
    press(&mut harness, at);
    release(&mut harness, at);
    harness.step();
    assert_eq!(
        board(&harness, &id).debug_selected_wires(),
        std::slice::from_ref(&line.id),
        "the line is selected"
    );
    let shown: Vec<String> = board(&harness, &id)
        .debug_statements()
        .into_iter()
        .map(|(text, _)| text)
        .collect();
    assert!(
        shown.iter().any(|text| text == "`hold` follows `first`"),
        "{shown:?}"
    );
}

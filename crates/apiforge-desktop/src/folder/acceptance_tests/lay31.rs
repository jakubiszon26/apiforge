//! LAY-31 (#384): a step and its checks are one group. Every card hung under
//! a step — its checks, and the values and data conditions that hang the
//! same way — sits 24 px under the card above it, and no line of another
//! card enters the group. In *Chips for shared values* a card hung under its
//! source reads it through a chip, with no line inside the group (owner's
//! decision 1 = A of 2026-11-17); dragging a step takes the cards hung under
//! it along (decision 2 = A). On the reference scenarios of
//! `fixtures/scenarios/lay/`, tidied.
use super::lay23::{Checks, fixture, graph, handle_of, id_of, show};
use super::*;
use crate::canvas::WireMode;
use apiforge_core::format::Graph;
use apiforge_layout::{Breach, GAP, Near, Point, Route, packed};
use egui::{Pos2, Rect, vec2};
use std::collections::{BTreeMap, BTreeSet};

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const DIAGRAM: &str = ".apiforge/diagram.json";
const TIDY: &str = "Tidy up";
const FIT: &str = "Fit the view";
/// The free space above a hung card (LAY-06's stack gap).
const STACK_GAP: f32 = 24.0;
const REFERENCE: [&str; 5] = ["shop-30", "shop-50", "order-flow", "fan-out", "far-value"];

/// The reference scenario `name` opened without positions, its checks as
/// `checks` says and its wires as `wires` says, tidied up once more with
/// nothing selected.
fn tidied(
    name: &str,
    size: egui::Vec2,
    checks: Checks,
    wires: WireMode,
) -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), fixture(name)).unwrap();
    fs::remove_file(project.path().join(DIAGRAM)).unwrap();
    let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).debug_wires(wires);
    if checks == Checks::Expanded {
        app(&mut harness).debug_draw_checks();
    }
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let id = only(&harness, DocumentKind::Scenario);
    app(&mut harness).selections.insert(id.clone(), Vec::new());
    settle(&mut harness);
    choose(&mut harness, TIDY);
    harness.step();
    (project, harness, id)
}

fn board<'a>(harness: &'a FolderHarness, id: &str) -> &'a crate::canvas::Canvas {
    folder(harness)
        .canvases
        .get(id)
        .expect("the board is drawn")
}

/// Every drawn card's box on the board, by node id.
fn cards(harness: &FolderHarness, id: &str) -> BTreeMap<String, Rect> {
    let canvas = board(harness, id);
    graph(harness, id)
        .nodes
        .iter()
        .filter_map(|node| Some((node.id.clone(), canvas.debug_world(&node.id)?)))
        .collect()
}

fn spine_step(graph: &Graph, node: &str) -> bool {
    graph
        .nodes
        .iter()
        .find(|candidate| candidate.id == node)
        .is_some_and(|node| crate::canvas::shape::on_spine(node.kind))
}

/// The card right above a card that is not a spine step: the nearest drawn
/// card above it in its column. On a board in shape D that is the card
/// above it in its stack, however far apart they are.
fn above(graph: &Graph, cards: &BTreeMap<String, Rect>, node: &str) -> Option<String> {
    if spine_step(graph, node) {
        return None;
    }
    let rect = cards.get(node)?;
    cards
        .iter()
        .filter(|(other, card)| {
            other.as_str() != node
                && (card.left() - rect.left()).abs() < 0.5
                && card.bottom() <= rect.top() + 0.5
        })
        .max_by(|a, b| a.1.bottom().total_cmp(&b.1.bottom()))
        .map(|(other, _)| other.clone())
}

/// The stacks of a board in shape D: each spine step with the cards hung
/// under it, as drawn, by node id; only stacks of two cards or more.
fn stacks(graph: &Graph, cards: &BTreeMap<String, Rect>) -> BTreeMap<String, Vec<String>> {
    let mut stacks: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for node in cards.keys() {
        let mut root = node.clone();
        while let Some(up) = above(graph, cards, &root) {
            root = up;
        }
        stacks.entry(root).or_default().push(node.clone());
    }
    stacks.retain(|_, members| members.len() > 1);
    stacks
}

/// The stack a card is in, by its spine step.
fn stack_of(stacks: &BTreeMap<String, Vec<String>>, node: &str) -> Option<String> {
    stacks
        .iter()
        .find(|(_, members)| members.iter().any(|member| member == node))
        .map(|(root, _)| root.clone())
}

fn bounds(cards: &BTreeMap<String, Rect>, members: &[String]) -> Rect {
    members
        .iter()
        .fold(Rect::NOTHING, |all, member| all.union(cards[member]))
}

/// Whether the segment from `a` to `b` has a piece inside `area`.
fn enters(a: Pos2, b: Pos2, area: Rect) -> bool {
    let segment = Rect::from_two_pos(a, b);
    segment.left() < area.right()
        && segment.right() > area.left()
        && segment.top() < area.bottom()
        && segment.bottom() > area.top()
}

/// Criterion 1: after Tidy up of each reference scenario, checks drawn, in
/// both *Wires* settings, the top of every hung card is exactly 24 px under
/// the card above it in its stack.
#[test]
fn lay31_criterion_1_a_hung_card_is_24_px_under_the_card_above() {
    let mut wrong = Vec::new();
    for name in REFERENCE {
        for wires in [WireMode::All, WireMode::Chips] {
            let (_project, harness, id) = tidied(name, WIDE, Checks::Expanded, wires);
            let graph = graph(&harness, &id);
            let cards = cards(&harness, &id);
            let mut hung = 0;
            for (node, rect) in &cards {
                let Some(up) = above(&graph, &cards, node) else {
                    continue;
                };
                hung += 1;
                let gap = rect.top() - cards[&up].bottom();
                if (gap - STACK_GAP).abs() > 0.5 {
                    wrong.push(format!(
                        "{name} {wires:?}: {} is {gap} px under {}",
                        handle_of(&graph, node),
                        handle_of(&graph, &up)
                    ));
                }
            }
            assert!(hung > 0, "{name} {wires:?} hangs cards under its steps");
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// Criterion 2: on the same boards no piece of a line whose two ends are
/// outside a group lies inside the group's box.
#[test]
fn lay31_criterion_2_no_line_of_another_card_enters_a_group() {
    let mut entered = Vec::new();
    for name in REFERENCE {
        for wires in [WireMode::All, WireMode::Chips] {
            let (_project, harness, id) = tidied(name, WIDE, Checks::Expanded, wires);
            let graph = graph(&harness, &id);
            let cards = cards(&harness, &id);
            let stacks = stacks(&graph, &cards);
            for (root, members) in &stacks {
                let area = bounds(&cards, members).shrink(0.5);
                for wire in board(&harness, &id).debug_wires() {
                    let own = members.contains(&wire.target)
                        || wire
                            .source
                            .as_ref()
                            .is_some_and(|source| members.contains(source));
                    if own || wire.revealed {
                        continue;
                    }
                    if wire
                        .points
                        .windows(2)
                        .any(|pair| enters(pair[0], pair[1], area))
                    {
                        entered.push(format!(
                            "{name} {wires:?}: {} -> {} enters the group of {}",
                            wire.source
                                .as_deref()
                                .map_or_else(|| "a chip".to_owned(), |s| handle_of(&graph, s)),
                            handle_of(&graph, &wire.target),
                            handle_of(&graph, root)
                        ));
                    }
                }
            }
        }
    }
    assert!(entered.is_empty(), "{entered:#?}");
}

/// Criterion 3: in *Chips for shared values* no line is drawn at rest from
/// a card to a card hung under it in its stack; that card's input has a
/// chip naming the value and the source's port counts `1 reader`. Pointing
/// at the chip, and selecting either card, draws the line.
#[test]
fn lay31_criterion_3_a_hung_card_reads_its_step_through_a_chip() {
    let (_project, mut harness, id) = tidied("shop-30", WIDE, Checks::Expanded, WireMode::Chips);
    let graph = graph(&harness, &id);
    let cards = cards(&harness, &id);
    let stacks = stacks(&graph, &cards);
    for wire in board(&harness, &id).debug_wires() {
        let Some(source) = wire.source.as_deref() else {
            continue;
        };
        if wire.revealed || wire.dashed {
            continue;
        }
        let (Some(from), Some(to)) = (cards.get(source), cards.get(&wire.target)) else {
            continue;
        };
        let hung = stack_of(&stacks, source).is_some()
            && stack_of(&stacks, source) == stack_of(&stacks, &wire.target)
            && !spine_step(&graph, &wire.target)
            && to.top() >= from.bottom();
        assert!(
            !hung,
            "no line from {} to {}, hung under it",
            handle_of(&graph, source),
            handle_of(&graph, &wire.target)
        );
    }
    let login = id_of(&harness, &id, "login");
    let logged_in = id_of(&harness, &id, "logged_in");
    let tag = board(&harness, &id)
        .debug_tags()
        .iter()
        .find(|tag| tag.tag.reader == logged_in && tag.tag.text == "login.status")
        .cloned()
        .expect("logged_in reads login.status through a chip");
    let count = board(&harness, &id)
        .debug_counts()
        .iter()
        .find(|count| count.count.source == login && count.count.port == tag.tag.port)
        .cloned()
        .expect("login's port counts its reader");
    assert_eq!(count.count.readers, 1);
    assert_eq!(count.count.label, "login.status → 1 reader");

    show(&mut harness, &id, "logged_in", 1.0);
    harness.get_by_label(&tag.tag.statement).hover();
    harness.step();
    harness.step();
    let shown = super::lay23::revealed(&harness, &id);
    assert!(
        tag.tag.wires.iter().all(|wire| shown.contains(wire)),
        "pointing at the chip draws the line: {shown:?}"
    );
    harness.remove_cursor();
    harness.step();
    for selected in ["logged_in", "login"] {
        super::lay23::select(&mut harness, &id, &[selected]);
        let shown = super::lay23::revealed(&harness, &id);
        assert!(
            tag.tag.wires.iter().all(|wire| shown.contains(wire)),
            "{selected} selected draws the line: {shown:?}"
        );
    }
}

/// The breaches of the 20 px gap among the lines drawn at rest that are not
/// a group's own: a line beside a card of its own group, and two lines of
/// one group near each other in or beside its box, are not counted.
fn breaches_outside_groups(harness: &FolderHarness, id: &str) -> Vec<(String, Breach)> {
    let graph = graph(harness, id);
    let cards = cards(harness, id);
    let stacks = stacks(&graph, &cards);
    let canvas = board(harness, id);
    let drawn: Vec<&crate::canvas::DrawnWire> = canvas
        .debug_wires()
        .iter()
        .filter(|wire| !wire.revealed && !wire.points.is_empty())
        .collect();
    let routes: Vec<Route> = drawn
        .iter()
        .map(|wire| Route {
            points: wire
                .points
                .iter()
                .map(|point| Point::new(point.x, point.y))
                .collect(),
            ..Route::default()
        })
        .collect();
    let rectangles = canvas.debug_geometry().0.clone();
    let names: Vec<String> = canvas
        .debug_cards()
        .iter()
        .map(|(card, _)| card.clone())
        .collect();
    let groups_of = |route: usize| -> BTreeSet<String> {
        let wire = drawn[route];
        [Some(wire.target.as_str()), wire.source.as_deref()]
            .into_iter()
            .flatten()
            .filter_map(|node| stack_of(&stacks, node))
            .collect()
    };
    packed(&routes, &rectangles, GAP)
        .into_iter()
        .filter(|breach| {
            let own = groups_of(breach.route);
            match breach.near {
                Near::Card(card) => !names
                    .get(card)
                    .and_then(|name| stack_of(&stacks, name))
                    .is_some_and(|group| own.contains(&group)),
                Near::Route(other) => !groups_of(other).intersection(&own).any(|group| {
                    let area = bounds(&cards, &stacks[group]).expand(GAP);
                    let centre = breach.area.center();
                    area.contains(egui::pos2(centre.x, centre.y))
                }),
            }
        })
        .map(|breach| {
            let wire = drawn[breach.route];
            (
                format!(
                    "{} -> {}",
                    wire.source
                        .as_deref()
                        .map_or_else(|| "a chip".to_owned(), |s| handle_of(&graph, s)),
                    handle_of(&graph, &wire.target)
                ),
                breach,
            )
        })
        .collect()
}

/// Criterion 4: in *All wires* a line between two cards of one group may
/// run nearer than 20 px to that group's cards; it passes under no card,
/// and it keeps 20 px from every card and line outside the group.
#[test]
fn lay31_criterion_4_inside_a_group_a_line_may_run_near_its_cards() {
    let mut found = Vec::new();
    for name in REFERENCE {
        let (_project, harness, id) = tidied(name, WIDE, Checks::Expanded, WireMode::All);
        let graph = graph(&harness, &id);
        let cards = cards(&harness, &id);
        for wire in board(&harness, &id).debug_wires() {
            for pair in wire.points.windows(2) {
                for (node, card) in &cards {
                    assert!(
                        !enters(pair[0], pair[1], card.shrink(1.0)),
                        "{name}: {} -> {} passes under {}",
                        wire.source
                            .as_deref()
                            .map_or_else(|| "a chip".to_owned(), |s| handle_of(&graph, s)),
                        handle_of(&graph, &wire.target),
                        handle_of(&graph, node)
                    );
                }
            }
        }
        found.extend(
            breaches_outside_groups(&harness, &id)
                .into_iter()
                .map(|(wire, breach)| format!("{name}: {wire} {breach:?}")),
        );
    }
    assert!(found.is_empty(), "{found:#?}");
}

/// Criterion 5: a check with several subjects sits under its latest
/// subject (`receipt_total` under `receipt`) and reads the other through a
/// chip or a line, as the *Wires* setting says.
#[test]
fn lay31_criterion_5_a_check_of_several_steps_hangs_under_the_latest() {
    for wires in [WireMode::All, WireMode::Chips] {
        let (_project, harness, id) = tidied("shop-30", WIDE, Checks::Expanded, wires);
        let graph = graph(&harness, &id);
        let cards = cards(&harness, &id);
        let stacks = stacks(&graph, &cards);
        let (receipt, total) = (
            id_of(&harness, &id, "receipt"),
            id_of(&harness, &id, "receipt_total"),
        );
        assert_eq!(
            stack_of(&stacks, &total),
            Some(receipt),
            "{wires:?}: receipt_total hangs under receipt"
        );
        let lines = super::lay23::wires(&harness, &id);
        let chips = super::lay23::chips(&harness, &id);
        let line = lines.contains(&("cart".to_owned(), "receipt_total".to_owned()));
        let chip = chips.contains(&("receipt_total".to_owned(), "cart.total".to_owned()));
        match wires {
            WireMode::All => assert!(line && !chip, "{wires:?}: {lines:?} {chips:?}"),
            WireMode::Chips => assert!(chip && !line, "{wires:?}: {lines:?} {chips:?}"),
        }
    }
}

/// Criterion 6: `shop-30`, checks drawn, the default view, 1600 × 1000: Fit
/// is not below 46 % and the crossings are not above 6. The lines,
/// crossings, coincident pairs and Fit of the five scenarios, checks drawn
/// and folded, are printed for the pull request.
#[test]
fn lay31_criterion_6_fit_and_crossings_on_shop_30() {
    for name in REFERENCE {
        for checks in [Checks::Expanded, Checks::Collapsed] {
            let (_project, mut harness, id) = tidied(name, WIDE, checks, WireMode::Chips);
            choose(&mut harness, FIT);
            harness.step();
            let (lines, crossings, pairs) = super::lay23::measures(&harness, &id);
            let fit = board(&harness, &id).debug_zoom();
            errln!(
                "LAY-31 {name} {checks:?}: {lines} lines, {crossings} crossings, {pairs} coincident pairs, Fit {:.0} %",
                fit * 100.0
            );
            if name == "shop-30" && checks == Checks::Expanded {
                assert!(fit >= 0.46 - 1e-3, "Fit {fit}");
                assert!(crossings <= 6, "{crossings} crossings");
            }
        }
    }
}

/// Criterion 8: with checks folded *Expand checks here* hangs the checks at
/// the same 24 px.
#[test]
fn lay31_criterion_8_expand_here_hangs_at_the_stack_gap() {
    let (_project, mut harness, id) = tidied("shop-30", WIDE, Checks::Collapsed, WireMode::Chips);
    show(&mut harness, &id, "cart", 1.0);
    harness.get_by_label("Checks of cart").click();
    settle(&mut harness);
    harness.step();
    harness.get_by_label("Expand checks here").click();
    settle(&mut harness);
    harness.step();
    let cards = cards(&harness, &id);
    let mut upper = cards[&id_of(&harness, &id, "cart")];
    for check in ["cart_created", "cart_total"] {
        let rect = cards[&id_of(&harness, &id, check)];
        assert_eq!(rect.top() - upper.bottom(), STACK_GAP, "{check}");
        upper = rect;
    }
}

/// Drags the card `handle` by `by` on screen, from its header.
fn drag_card(harness: &mut FolderHarness, id: &str, handle: &str, by: egui::Vec2) {
    let node = id_of(harness, id, handle);
    let card = board(harness, id)
        .debug_card_rect(&node)
        .unwrap_or_else(|| panic!("{handle} is drawn"));
    let from = card.center_top() + vec2(0.0, 12.0);
    super::lay16::drag(harness, from, from + by);
    settle(harness);
    harness.step();
}

/// Criterion 10: dragging `cart` by 200 px moves `cart_created` and
/// `cart_total` by the same offset, in one undo step; dragging `cart_total`
/// alone moves only it; a card dragged out of the stack is then drawn as
/// any card that is not under its subject: `cart_created` reads
/// `cart.status` through a line again.
#[test]
fn lay31_criterion_10_dragging_a_step_takes_its_hung_cards() {
    let (_project, mut harness, id) = tidied("shop-30", WIDE, Checks::Expanded, WireMode::Chips);
    show(&mut harness, &id, "cart", 1.0);
    let before = cards(&harness, &id);
    let of = |harness: &FolderHarness, handle: &str| id_of(harness, &id, handle);
    let group = ["cart", "cart_created", "cart_total"].map(|handle| of(&harness, handle));
    drag_card(&mut harness, &id, "cart", vec2(200.0, 0.0));
    let after = cards(&harness, &id);
    for (node, rect) in &before {
        let moved = after[node].min - rect.min;
        if group.contains(node) {
            assert_eq!(
                moved,
                vec2(200.0, 0.0),
                "{} moves with cart",
                handle_of(&graph(&harness, &id), node)
            );
        } else {
            assert_eq!(
                moved,
                vec2(0.0, 0.0),
                "{} stays",
                handle_of(&graph(&harness, &id), node)
            );
        }
    }
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    settle(&mut harness);
    harness.step();
    assert_eq!(
        cards(&harness, &id),
        before,
        "one undo step puts all three back"
    );

    drag_card(&mut harness, &id, "cart_total", vec2(200.0, 0.0));
    let after = cards(&harness, &id);
    for (node, rect) in &before {
        let moved = after[node].min - rect.min;
        let expected = if *node == group[2] {
            vec2(200.0, 0.0)
        } else {
            vec2(0.0, 0.0)
        };
        assert_eq!(
            moved,
            expected,
            "{}",
            handle_of(&graph(&harness, &id), node)
        );
    }

    let cart_status = ("cart".to_owned(), "cart_created".to_owned());
    assert!(!super::lay23::wires(&harness, &id).contains(&cart_status));
    drag_card(&mut harness, &id, "cart_created", vec2(-200.0, 0.0));
    assert!(
        super::lay23::wires(&harness, &id).contains(&cart_status),
        "out of the stack, cart_created reads cart.status through a line: {:?}",
        super::lay23::wires(&harness, &id)
    );
}

/// Criterion 9: a board with saved positions opens with its cards where
/// they were: a check dragged out of its stack and saved opens where it
/// was put, and nothing else moves.
#[test]
fn lay31_criterion_9_a_saved_board_opens_where_it_was() {
    let (project, mut harness, id) = tidied("shop-30", WIDE, Checks::Expanded, WireMode::Chips);
    show(&mut harness, &id, "cart", 1.0);
    drag_card(&mut harness, &id, "cart_total", vec2(0.0, 56.0));
    choose(&mut harness, "Save");
    settle(&mut harness);
    let saved: BTreeMap<String, apiforge_core::format::Position> = graph(&harness, &id)
        .nodes
        .iter()
        .map(|node| (node.handle.clone(), node.position.clone()))
        .collect();
    drop(harness);
    let mut again = crate::testing::folder_harness_in(project.path(), WIDE, Mode::Diagram);
    app(&mut again).review_open(SCENARIO, None);
    settle(&mut again);
    again.step();
    let id = only(&again, DocumentKind::Scenario);
    for node in &graph(&again, &id).nodes {
        assert_eq!(
            Some(&node.position),
            saved.get(&node.handle),
            "{} opens where it was",
            node.handle
        );
    }
}

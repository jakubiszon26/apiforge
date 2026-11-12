//! LAY-23 mock-up (#333): throwaway views of the branch `mock/lay-23-fan-out`,
//! never merged. Each direction is rendered on the synthetic `fan-out`
//! scenario and on `shop-30`, tidied, at Fit and at 60 %, and measured with
//! the definitions of the `quality` example (LAY-01, LAY-06): crossings,
//! close and coincident pairs, straight wires, and the length of what is
//! drawn.
use super::*;
use crate::canvas::lay23::{Config, Drawn, Mode as Wires, Order};
use egui::Pos2;
use std::fmt::Write as _;

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const DIAGRAM: &str = ".apiforge/diagram.json";
const TIDY: &str = "Tidy up";
const FIT: &str = "Fit the view";

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios/lay")
        .join(format!("{name}.afs"));
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the fixture {}: {error}", path.display()))
}

/// The scenario `name` opened without positions and tidied, its checks
/// collapsed (the default) or drawn.
fn tidied(name: &str, expanded: bool) -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), fixture(name)).unwrap();
    fs::remove_file(project.path().join(DIAGRAM)).unwrap();
    let mut harness = crate::testing::folder_harness_in(project.path(), WIDE, Mode::Diagram);
    if expanded {
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

fn id_of(harness: &FolderHarness, id: &str, handle: &str) -> String {
    let snapshot = folder(harness).snapshot(id).unwrap();
    graph_of(snapshot.model.as_ref().unwrap(), false)
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("a step {handle}"))
        .id
        .clone()
}

fn set(harness: &mut FolderHarness, id: &str, config: Config) {
    app(harness).canvases.get_mut(id).unwrap().lay23 = config;
    settle(harness);
    harness.step();
}

fn select(harness: &mut FolderHarness, id: &str, handles: &[&str]) {
    let ids = handles
        .iter()
        .map(|handle| id_of(harness, id, handle))
        .collect();
    app(harness).selections.insert(id.to_owned(), ids);
    settle(harness);
    harness.step();
}

fn drawn(harness: &FolderHarness, id: &str) -> Drawn {
    folder(harness)
        .canvases
        .get(id)
        .unwrap()
        .lay23_drawn
        .clone()
}

type Segment = (Pos2, Pos2);

fn horizontal(s: &Segment) -> bool {
    (s.0.y - s.1.y).abs() < 0.01
}

fn vertical(s: &Segment) -> bool {
    (s.0.x - s.1.x).abs() < 0.01
}

/// The segments of a net with what its paths share counted once: collinear
/// overlapping pieces are merged.
fn union(paths: &[Vec<Pos2>]) -> Vec<Segment> {
    let mut lines: Vec<(bool, i64, f32, f32)> = Vec::new();
    for path in paths {
        for pair in path.windows(2) {
            let s = (pair[0], pair[1]);
            if horizontal(&s) {
                lines.push((
                    true,
                    (s.0.y * 10.0).round() as i64,
                    s.0.x.min(s.1.x),
                    s.0.x.max(s.1.x),
                ));
            } else if vertical(&s) {
                lines.push((
                    false,
                    (s.0.x * 10.0).round() as i64,
                    s.0.y.min(s.1.y),
                    s.0.y.max(s.1.y),
                ));
            }
        }
    }
    lines.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)).then(a.2.total_cmp(&b.2)));
    let mut merged: Vec<(bool, i64, f32, f32)> = Vec::new();
    for line in lines {
        if line.3 - line.2 < 0.01 {
            continue;
        }
        match merged.last_mut() {
            Some(last) if last.0 == line.0 && last.1 == line.1 && line.2 <= last.3 + 0.01 => {
                last.3 = last.3.max(line.3);
            }
            _ => merged.push(line),
        }
    }
    merged
        .into_iter()
        .map(|(across, at, from, to)| {
            let at = at as f32 / 10.0;
            if across {
                (Pos2::new(from, at), Pos2::new(to, at))
            } else {
                (Pos2::new(at, from), Pos2::new(at, to))
            }
        })
        .collect()
}

struct Measures {
    /// Lines a person tells apart at the source: a routed wire, or a bus.
    lines: usize,
    data: usize,
    order: usize,
    buses: usize,
    chips: usize,
    hidden: usize,
    crossings: usize,
    close: usize,
    coincident: usize,
    straight: usize,
    /// World pixels of line drawn.
    ink: f32,
    longest: f32,
}

fn length(segments: &[Segment]) -> f32 {
    segments.iter().map(|s| s.0.distance(s.1)).sum()
}

fn measure(drawn: &Drawn) -> Measures {
    // Each net is one wire's route, or one bus with its shared parts once.
    let mut nets: Vec<Vec<Segment>> = drawn
        .routed
        .iter()
        .map(|(points, _)| union(std::slice::from_ref(points)))
        .collect();
    nets.extend(drawn.buses.iter().map(|paths| union(paths)));
    let (mut crossings, mut close, mut coincident) = (0, 0, 0);
    for (index, a) in nets.iter().enumerate() {
        for b in &nets[index + 1..] {
            for p in a {
                for q in b {
                    let (h, v) = if horizontal(p) && vertical(q) {
                        (p, q)
                    } else if vertical(p) && horizontal(q) {
                        (q, p)
                    } else {
                        let overlap = |a1: f32, a2: f32, b1: f32, b2: f32| {
                            a1.max(a2).min(b1.max(b2)) - a1.min(a2).max(b1.min(b2))
                        };
                        let (gap, along) = if horizontal(p) {
                            ((p.0.y - q.0.y).abs(), overlap(p.0.x, p.1.x, q.0.x, q.1.x))
                        } else {
                            ((p.0.x - q.0.x).abs(), overlap(p.0.y, p.1.y, q.0.y, q.1.y))
                        };
                        if along >= 20.0 {
                            if gap < 0.5 {
                                coincident += 1;
                            } else if gap <= 8.0 {
                                close += 1;
                            }
                        }
                        continue;
                    };
                    let (x, y) = (v.0.x, h.0.y);
                    if x > h.0.x.min(h.1.x) + 0.5
                        && x < h.0.x.max(h.1.x) - 0.5
                        && y > v.0.y.min(v.1.y) + 0.5
                        && y < v.0.y.max(v.1.y) - 0.5
                    {
                        crossings += 1;
                    }
                }
            }
        }
    }
    let lengths: Vec<f32> = nets.iter().map(|net| length(net)).collect();
    Measures {
        lines: nets.len(),
        data: drawn.routed.iter().filter(|(_, order)| !order).count(),
        order: drawn.routed.iter().filter(|(_, order)| *order).count(),
        buses: drawn.buses.len(),
        chips: drawn.tags.len(),
        hidden: drawn.hidden,
        crossings,
        close,
        coincident,
        straight: drawn
            .routed
            .iter()
            .filter(|(points, _)| union(std::slice::from_ref(points)).len() == 1)
            .count(),
        ink: lengths.iter().sum(),
        longest: lengths.iter().copied().fold(0.0, f32::max),
    }
}

const HEAD: &str = "| board | checks | option | lines | data wires | order wires | buses | chips | not drawn | crossings | close | coincident | straight | ink px | longest px |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n";

fn row(text: &mut String, board: &str, checks: &str, option: &str, m: &Measures) {
    writeln!(
        text,
        "| {board} | {checks} | {option} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {:.0} | {:.0} |",
        m.lines,
        m.data,
        m.order,
        m.buses,
        m.chips,
        m.hidden,
        m.crossings,
        m.close,
        m.coincident,
        m.straight,
        m.ink,
        m.longest
    )
    .unwrap();
}

fn count(mode: Wires, min_readers: usize, keep_near: bool) -> Config {
    Config {
        mode,
        min_readers,
        keep_near,
        every_far_wire: false,
        order: Order::All,
        merge_parallel: false,
    }
}

fn options() -> Vec<(&'static str, Config)> {
    vec![
        ("main", Config::default()),
        (
            "A chips, 3+ readers, near kept",
            count(Wires::Chips, 3, true),
        ),
        ("A chips, 3+ readers, all", count(Wires::Chips, 3, false)),
        (
            "A chips, 2+ readers, near kept",
            count(Wires::Chips, 2, true),
        ),
        (
            "A chips, 4+ readers, near kept",
            count(Wires::Chips, 4, true),
        ),
        (
            "A chips, every wire past the next column",
            Config {
                mode: Wires::Chips,
                min_readers: 1,
                keep_near: true,
                every_far_wire: true,
                order: Order::All,
                merge_parallel: false,
            },
        ),
        ("B bus, 3+ readers, near kept", count(Wires::Bus, 3, true)),
        ("B bus, 3+ readers, all", count(Wires::Bus, 3, false)),
        ("C on demand", count(Wires::OnDemand, 3, true)),
        (
            "D variables with proxy chips, 3+ readers",
            count(Wires::Proxies, 3, true),
        ),
        (
            "today, without @after the data implies",
            Config {
                order: Order::WithoutImplied,
                ..Config::default()
            },
        ),
        (
            "today, without order wires into the next step",
            Config {
                order: Order::WithoutNext,
                ..Config::default()
            },
        ),
        (
            "today, one wire per port and card",
            Config {
                merge_parallel: true,
                ..Config::default()
            },
        ),
        (
            "A + one wire per port and card",
            Config {
                merge_parallel: true,
                ..count(Wires::Chips, 3, true)
            },
        ),
        (
            "A + without order wires into the next step",
            Config {
                order: Order::WithoutNext,
                ..count(Wires::Chips, 3, true)
            },
        ),
        (
            "A + both",
            Config {
                order: Order::WithoutNext,
                merge_parallel: true,
                ..count(Wires::Chips, 3, true)
            },
        ),
        (
            "C + without order wires into the next step",
            Config {
                order: Order::WithoutNext,
                ..count(Wires::OnDemand, 3, true)
            },
        ),
    ]
}

/// The measurements table of the analysis, written beside the captures.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay23_measures() {
    let mut text = String::from(HEAD);
    for name in ["fan-out", "shop-30", "shop-30-vars"] {
        for expanded in [false, true] {
            let (_project, mut harness, id) = tidied(name, expanded);
            let checks = if expanded { "expanded" } else { "collapsed" };
            for (label, config) in options() {
                set(&mut harness, &id, config);
                let drawn = drawn(&harness, &id);
                let measures = measure(&drawn);
                if config.mode == Wires::Off {
                    // The geometric count beside the router's bridges.
                    let bridges: usize = folder(&harness)
                        .canvases
                        .get(&id)
                        .unwrap()
                        .debug_wires()
                        .iter()
                        .map(|wire| wire.bridges)
                        .sum();
                    writeln!(
                        text,
                        "<!-- {name} {checks}: the router's bridges {bridges}, counted {} -->",
                        measures.crossings
                    )
                    .unwrap();
                }
                row(&mut text, name, checks, label, &measures);
            }
            // With a card selected: what comes back.
            let source = match name {
                "fan-out" => "cartId",
                "shop-30" => "login",
                _ => "token",
            };
            for (label, mode, handle) in [
                ("A, a reader selected", Wires::Chips, "checkout"),
                ("A, the source selected", Wires::Chips, source),
                ("C, a reader selected", Wires::OnDemand, "checkout"),
                ("C, the source selected", Wires::OnDemand, source),
            ] {
                set(
                    &mut harness,
                    &id,
                    Config {
                        mode,
                        ..Config::default()
                    },
                );
                select(&mut harness, &id, &[handle]);
                let measures = measure(&drawn(&harness, &id));
                row(&mut text, name, checks, label, &measures);
                select(&mut harness, &id, &[]);
            }
        }
    }
    let directory = crate::testing::capture_dir();
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("lay23-measures.md"), &text).unwrap();
    println!("{text}");
}

fn assert_in_view(harness: &FolderHarness, id: &str, label: &str) {
    let canvas = folder(harness).canvases.get(id).unwrap();
    let focus = canvas.debug_focus();
    assert!(
        canvas
            .debug_cards()
            .iter()
            .any(|(_, rect)| focus.contains_rect(*rect)),
        "{label}: a card is in view"
    );
}

#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay23_captures() {
    // Direction D: the variable style with proxy chips.
    for (name, centre) in [("fan-out", "shipping"), ("shop-30-vars", "address")] {
        let (_project, mut harness, id) = tidied(name, false);
        for (view, config) in [
            ("d-main", Config::default()),
            ("d-proxies", count(Wires::Proxies, 3, true)),
        ] {
            if name == "fan-out" && view == "d-main" {
                continue;
            }
            set(&mut harness, &id, config);
            choose(&mut harness, FIT);
            harness.step();
            let label = format!("lay23-{name}-{view}-fit");
            assert_in_view(&harness, &id, &label);
            crate::testing::capture(&mut harness, &label, None);
            let card = id_of(&harness, &id, centre);
            app(&mut harness)
                .canvases
                .get_mut(&id)
                .unwrap()
                .debug_show(&card, 0.6);
            settle(&mut harness);
            harness.step();
            let label = format!("lay23-{name}-{view}-60");
            assert_in_view(&harness, &id, &label);
            crate::testing::capture(&mut harness, &label, None);
        }
    }
    for (name, centre, reader, source) in [
        ("fan-out", "shipping", "checkout", "cartId"),
        ("shop-30", "address", "checkout", "login"),
    ] {
        let (_project, mut harness, id) = tidied(name, false);
        let chips = count(Wires::Chips, 3, true);
        let views: Vec<(&str, Config, Vec<&str>)> = vec![
            ("main", Config::default(), vec![]),
            ("a-chips", chips, vec![]),
            ("a-chips-reader-selected", chips, vec![reader]),
            ("a-chips-source-selected", chips, vec![source]),
            (
                "a-chips-no-next-rails",
                Config {
                    order: Order::WithoutNext,
                    ..chips
                },
                vec![],
            ),
            ("b-bus", count(Wires::Bus, 3, true), vec![]),
            ("c-on-demand", count(Wires::OnDemand, 3, true), vec![]),
            (
                "c-on-demand-reader-selected",
                count(Wires::OnDemand, 3, true),
                vec![reader],
            ),
        ];
        // Checks drawn: the board LAY-03's cap is measured on.
        {
            let (_project, mut harness, id) = tidied(name, true);
            for (view, config) in [("main", Config::default()), ("a-chips", chips)] {
                set(&mut harness, &id, config);
                choose(&mut harness, FIT);
                harness.step();
                let label = format!("lay23-{name}-expanded-{view}-fit");
                assert_in_view(&harness, &id, &label);
                crate::testing::capture(&mut harness, &label, None);
            }
        }
        for (view, config, selection) in views {
            set(&mut harness, &id, config);
            select(&mut harness, &id, &selection);
            choose(&mut harness, FIT);
            harness.step();
            let label = format!("lay23-{name}-{view}-fit");
            assert_in_view(&harness, &id, &label);
            crate::testing::capture(&mut harness, &label, None);
            // At 60 % the camera is on the selected card, or on the middle
            // of the board.
            let card = id_of(&harness, &id, selection.first().copied().unwrap_or(centre));
            app(&mut harness)
                .canvases
                .get_mut(&id)
                .unwrap()
                .debug_show(&card, 0.6);
            settle(&mut harness);
            harness.step();
            let label = format!("lay23-{name}-{view}-60");
            assert_in_view(&harness, &id, &label);
            crate::testing::capture(&mut harness, &label, None);
        }
    }
}

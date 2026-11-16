//! LAY-29 mock-up (throwaway): where an ordering wire leaves its card.
//! `APIFORGE_LAY29` is unset (as on `main`), `a` (from the socket at the
//! bottom, round the card) or `b` (a socket at the top, where the rail
//! leaves).
use super::*;
use crate::canvas::WireMode;
use apiforge_core::format::Graph;
use apiforge_layout::{GAP, Point, Route, crossing, packed, under_own};

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const DIAGRAM: &str = ".apiforge/diagram.json";
const TIDY: &str = "Tidy up";
const FIT: &str = "Fit the view";

const FIVE: &str = r#"scenario "Five after start" {
  begin = start
  @after(begin)
  login = POST "/login" {
    body { user: "demo", password: "demo" }
    sample 200 { token: "t-1" }
  }
  @after(begin)
  products = GET "/products" {
    sample 200 { items: [{ id: "p-1", price: 12.5 }] }
  }
  @after(begin)
  cart = POST "/cart" {
    body { items: [] }
    sample 201 { id: "c-1", total: 28.0 }
  }
  @after(begin)
  profile = GET "/profile" {
    sample 200 { name: "demo" }
  }
  @after(begin)
  orders = GET "/orders" {
    sample 200 { items: [] }
  }
}
"#;

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios/lay")
        .join(format!("{name}.afs"));
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the fixture {}: {error}", path.display()))
}

fn tidied(
    text: &str,
    size: egui::Vec2,
    expanded: bool,
    wires: WireMode,
) -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), text).unwrap();
    fs::remove_file(project.path().join(DIAGRAM)).unwrap();
    let mut harness = crate::testing::folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).debug_wires(wires);
    if expanded {
        app(&mut harness).debug_draw_checks();
    }
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let id = only(&harness, DocumentKind::Scenario);
    app(&mut harness).selections.insert(id.clone(), Vec::new());
    settle(&mut harness);
    choose(&mut harness, TIDY);
    settle(&mut harness);
    harness.step();
    (project, harness, id)
}

fn board<'a>(harness: &'a FolderHarness, id: &str) -> &'a crate::canvas::Canvas {
    folder(harness)
        .canvases
        .get(id)
        .expect("the board is drawn")
}

fn graph(harness: &FolderHarness, id: &str) -> Graph {
    let snapshot = folder(harness).snapshot(id).unwrap();
    graph_of(snapshot.model.as_ref().unwrap(), false).clone()
}

fn crossings(routes: &[Route]) -> usize {
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

#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay29_mock() {
    let variant = std::env::var("APIFORGE_LAY29").unwrap_or_else(|_| "main".to_owned());
    let five = FIVE.to_owned();
    let shop = fixture("shop-30");
    let fan = fixture("fan-out");
    let views = [
        ("five", &five, WireMode::Chips, false, Some("begin"), crate::testing::MINIMUM),
        ("five-fit", &five, WireMode::Chips, false, None, WIDE),
        ("shop-30-paid", &shop, WireMode::Chips, false, Some("paid"), crate::testing::MINIMUM),
        ("shop-30-fit", &shop, WireMode::Chips, false, None, WIDE),
        ("shop-30-all-fit", &shop, WireMode::All, true, None, WIDE),
        ("fan-out-all-fit", &fan, WireMode::All, false, None, WIDE),
    ];
    for (label, text, wires, expanded, centre, size) in views {
        let (_project, mut harness, id) = tidied(text, size, expanded, wires);
        choose(&mut harness, FIT);
        harness.step();
        if let Some(handle) = centre {
            let node = graph(&harness, &id)
                .nodes
                .iter()
                .find(|node| node.handle == handle)
                .unwrap_or_else(|| panic!("a step {handle}"))
                .id
                .clone();
            app(&mut harness)
                .canvases
                .get_mut(&id)
                .expect("the board is drawn")
                .debug_show(&node, 1.0);
            settle(&mut harness);
            harness.step();
        }
        let canvas = board(&harness, &id);
        let (cards, wires) = canvas.debug_geometry().clone();
        let routes: Vec<Route> = canvas
            .debug_wires()
            .iter()
            .filter(|wire| !wire.points.is_empty())
            .map(|wire| Route {
                points: wire
                    .points
                    .iter()
                    .map(|point| Point::new(point.x, point.y))
                    .collect(),
                ..Route::default()
            })
            .collect();
        let bends: usize = routes.iter().map(Route::bends).sum();
        let bounds = canvas
            .debug_cards()
            .iter()
            .map(|(_, rect)| *rect)
            .reduce(|a, b| a.union(b))
            .unwrap();
        let routed = apiforge_layout::route(&cards, &wires, &apiforge_layout::Routing::default());
        println!(
            "LAY29 {variant} {label}: lines {} crossings {} bends {} packed {} under {} board {:.0}x{:.0} zoom {:.3}",
            routes.len(),
            crossings(&routes),
            bends,
            packed(&routes, &cards, GAP).len(),
            under_own(&routed, &wires, &cards).len(),
            bounds.width(),
            bounds.height(),
            canvas.debug_zoom(),
        );
        let focus = canvas.debug_focus();
        assert!(
            canvas
                .debug_cards()
                .iter()
                .any(|(_, rect)| focus.contains_rect(*rect)),
            "{label}: a card is in view"
        );
        crate::testing::capture(&mut harness, &format!("lay29-{variant}-{label}"), None);
    }
}

//! LAY-16 mock-up views (throwaway branch `mock/lay-16-inputs`): a brand-new
//! request beside `cart` at rest, selected and during a wire drag, under each
//! option for the ports a compact card hides; and the facts about today's
//! wiring gestures, run in the harness and written to `lay16-facts.txt`.
use super::*;
use crate::mock_lay16::{self as mock, Flags, Option_};
use crate::testing::{
    FolderHarness, MINIMUM, WIDE, capture, capture_dir, folder, folder_harness_in, settle,
    shop_project,
};
use apiforge_core::format::Binding;
use eframe::egui::{self, Pos2, Rect, pos2, vec2};
use egui_kittest::kittest::Queryable;

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";

/// shop-30 with one more request, `probe`, as *Add step* leaves one: nothing
/// reads it, it reads nothing, no `@after`.
fn fixture() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios/lay/shop-30.afs");
    let text = std::fs::read_to_string(path).unwrap();
    let edited = text.replacen(
        "  cleanup {",
        "  probe = GET \"/probe\" {\n    sample 200 {}\n  }\n  cleanup {",
        1,
    );
    assert_ne!(edited, text);
    edited
}

fn note(line: &str) {
    let path = capture_dir().join("lay16-facts.txt");
    std::fs::create_dir_all(capture_dir()).unwrap();
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    writeln!(file, "{line}").unwrap();
}

fn app(harness: &mut FolderHarness) -> &mut FolderApp {
    harness.state_mut().as_mut().unwrap()
}

fn document(harness: &FolderHarness) -> String {
    folder(harness)
        .session
        .as_ref()
        .unwrap()
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == SCENARIO)
        .unwrap()
        .id()
        .to_owned()
}

fn graph(harness: &FolderHarness) -> Graph {
    let id = document(harness);
    let model = folder(harness).snapshot(&id).unwrap().model.unwrap();
    graph_of(&model, false).clone()
}

fn id_of(graph: &Graph, handle: &str) -> String {
    graph
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("no {handle}"))
        .id
        .clone()
}

fn open(size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, Graph) {
    mock::set(Flags::default());
    mock::clear();
    let project = shop_project();
    std::fs::write(project.path().join(SCENARIO), fixture()).unwrap();
    let mut harness = folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    harness.step();
    let graph = graph(&harness);
    (project, harness, graph)
}

/// A free origin for `probe` beside `cart`: right of it, or further down
/// the same column when a card is in the way.
fn beside(graph: &Graph, cart: Rect) -> Pos2 {
    let others: Vec<Rect> = graph
        .nodes
        .iter()
        .filter_map(|node| mock::world(&node.id))
        .collect();
    let mut candidates: Vec<(f32, Pos2)> = Vec::new();
    for dx in [380.0, 760.0, 1140.0] {
        for step in -4..=4 {
            let offset = vec2(dx, step as f32 * 130.0);
            candidates.push((offset.length(), cart.min + offset));
        }
    }
    candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, origin) in candidates {
        let candidate = Rect::from_min_size(origin, vec2(280.0, 200.0));
        if !others
            .iter()
            .any(|other| other.expand(60.0).intersects(candidate))
        {
            return origin;
        }
    }
    let right = others.iter().map(|r| r.right()).fold(0.0, f32::max);
    pos2(right + 120.0, cart.top())
}

/// The view: the option on, `probe` beside `cart`, the camera on both.
fn view(harness: &mut FolderHarness, graph: &Graph, option: Option_, zoom: f32) {
    let cart = id_of(graph, "cart");
    let probe = id_of(graph, "probe");
    let cart_rect = mock::world(&cart).expect("cart drawn");
    let origin = beside(graph, cart_rect);
    mock::set(Flags {
        option,
        camera: Some((zoom, vec![cart.clone(), probe.clone()])),
        place: vec![(probe, origin)],
    });
    settle(harness);
    harness.step();
    harness.step();
    // Placed once; from now on the camera is the application's, so that
    // selecting a card pans as on `main` when the Inspector opens.
    mock::update(|flags| flags.camera = None);
    harness.step();
}

fn screen(harness: &FolderHarness, id: &str) -> Rect {
    let _ = harness;
    mock::drawn(id).expect("card drawn").rect
}

/// The two cards and what the option draws beside them.
fn region(harness: &FolderHarness, graph: &Graph) -> Rect {
    let cart = screen(harness, &id_of(graph, "cart"));
    let probe = screen(harness, &id_of(graph, "probe"));
    cart.union(probe).expand2(vec2(110.0, 60.0))
}

fn press(harness: &mut FolderHarness, at: Pos2) {
    harness.hover_at(at);
    harness.step();
    harness.event(egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
}

fn move_to(harness: &mut FolderHarness, from: Pos2, to: Pos2) {
    let steps = ((to - from).length() / 24.0).ceil().max(2.0);
    for step in 1..=steps as usize {
        harness.hover_at(from + (to - from) * (step as f32 / steps));
        harness.step();
    }
    harness.step();
}

fn release(harness: &mut FolderHarness, at: Pos2) {
    harness.event(egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
    harness.remove_cursor();
    settle(harness);
    harness.step();
}

fn click(harness: &mut FolderHarness, at: Pos2) {
    press(harness, at);
    release(harness, at);
}

fn right_click(harness: &mut FolderHarness, at: Pos2) {
    harness.hover_at(at);
    harness.step();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.step();
    }
    harness.step();
}

/// A capture with the pointer where it is: a wire being dragged.
fn shot_held(harness: &mut FolderHarness, name: &str, region: Option<Rect>) {
    harness.step();
    let image = harness.render().unwrap();
    let image = match region {
        Some(region) => {
            let window = harness.ctx.content_rect();
            let pixels = region.intersect(window);
            let x = pixels.min.x.floor().max(0.0) as u32;
            let y = pixels.min.y.floor().max(0.0) as u32;
            let width = (pixels.width().ceil() as u32).min(image.width() - x);
            let height = (pixels.height().ceil() as u32).min(image.height() - y);
            image::imageops::crop_imm(&image, x, y, width, height).to_image()
        }
        None => image,
    };
    std::fs::create_dir_all(capture_dir()).unwrap();
    image
        .save(capture_dir().join(format!("{name}.png")))
        .unwrap();
}

fn has(harness: &FolderHarness, label: &str) -> bool {
    harness.query_by_label(label).is_some()
}

fn has_part(harness: &FolderHarness, part: &str) -> bool {
    harness.query_by_label_contains(part).is_some()
}

fn socket_center(harness: &FolderHarness, label: &str) -> Pos2 {
    harness.get_by_label(label).rect().center()
}

fn body_binding(harness: &FolderHarness, handle: &str) -> String {
    binding_of(harness, handle, "body")
}

fn binding_of(harness: &FolderHarness, handle: &str, input: &str) -> String {
    let graph = graph(harness);
    let node = graph
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap();
    match node.inputs.get(input) {
        None => "none".to_owned(),
        Some(Binding::Reference { node_id, port, .. }) => {
            let source = graph
                .nodes
                .iter()
                .find(|node| &node.id == node_id)
                .map_or(node_id.clone(), |node| node.handle.clone());
            format!("reference {source}.{port}")
        }
        Some(Binding::Literal { value }) => format!("literal {value}"),
        Some(other) => format!("{other:?}").chars().take(40).collect(),
    }
}

fn order_edge(harness: &FolderHarness, from: &str, to: &str) -> bool {
    let graph = graph(harness);
    let from = id_of(&graph, from);
    let to = id_of(&graph, to);
    graph
        .edges
        .iter()
        .any(|edge| edge.source == from && edge.target == to)
}

fn suffix(size: egui::Vec2) -> &'static str {
    if size == WIDE { "1600" } else { "960" }
}

fn name(option: Option_) -> &'static str {
    match option {
        Option_::Today => "today",
        Option_::A => "a",
        Option_::B => "b",
        Option_::C => "c",
        Option_::D => "d",
    }
}

/// Where a person aims the wire at `probe` under the option: the Body row
/// today and under C, the edge under A and B, the tray under D.
fn aim(option: Option_, probe: Rect, zoom: f32) -> Pos2 {
    match option {
        Option_::Today => pos2(probe.left(), probe.top() + 170.0 * zoom),
        Option_::A | Option_::B => pos2(probe.left(), probe.top() + 72.0 * zoom),
        Option_::C => pos2(probe.left(), probe.top() + 74.0 * zoom),
        Option_::D => pos2(probe.left() - 92.0 * zoom, probe.top() + 56.0 * zoom),
    }
}

/// Today's wiring gestures, verified in the harness: what the card offers at
/// 1.0 (Full) and at 0.6 (Compact), the data and order drags, the right-click
/// menu and the Inspector's connections.
#[test]
#[ignore = "design-review view"]
fn review_lay16_facts_today() {
    let (_project, mut harness, graph) = open(WIDE);
    let probe = id_of(&graph, "probe");
    for zoom in [0.6, 1.0] {
        view(&mut harness, &graph, Option_::Today, zoom);
        note(&format!(
            "today at {zoom:.1}: '+' button on cart: {}; Next button on cart: {}; output socket body of cart: {}; output socket body of probe: {}",
            has(&harness, "Add a step connected to cart"),
            has(&harness, "Add the next step (next) after cart"),
            has(&harness, "Output body of cart"),
            has(&harness, "Output body of probe"),
        ));
    }
    // Compact detail today, for the record.
    view(&mut harness, &graph, Option_::Today, 0.6);
    capture(&mut harness, "lay16-today-60-rest-1600", None);
    let cards = region(&harness, &graph);
    capture(&mut harness, "lay16-today-60-rest-1600-cards", Some(cards));

    view(&mut harness, &graph, Option_::Today, 0.8);
    capture(&mut harness, "lay16-today-rest-1600", None);
    let cards = region(&harness, &graph);
    capture(&mut harness, "lay16-today-rest-1600-cards", Some(cards));
    note(&format!(
        "today heights (world px): login {:.0}, cart {:.0}, probe {:.0}",
        mock::world(&id_of(&graph, "login")).unwrap().height(),
        mock::world(&id_of(&graph, "cart")).unwrap().height(),
        mock::world(&probe).unwrap().height(),
    ));

    // The data drag: cart's Response socket to probe's Body row.
    let from = socket_center(&harness, "Output body of cart");
    let to = aim(Option_::Today, screen(&harness, &probe), 0.8);
    press(&mut harness, from);
    move_to(&mut harness, from, to);
    shot_held(&mut harness, "lay16-today-drag-1600", None);
    let cards = region(&harness, &graph);
    shot_held(&mut harness, "lay16-today-drag-1600-cards", Some(cards));
    let target = mock::target();
    release(&mut harness, to);
    note(&format!(
        "today data drag cart.body → probe Body row: snapped target {target:?}; probe.body after release: {}",
        body_binding(&harness, "probe")
    ));

    // The order drag: cart's bottom socket to probe's top socket.
    let from = socket_center(&harness, "Output __order of cart");
    let to = screen(&harness, &probe).center_top();
    press(&mut harness, from);
    move_to(&mut harness, from, to);
    let target = mock::target();
    release(&mut harness, to);
    note(&format!(
        "today order drag cart → probe: snapped target {target:?}; edge cart→probe after release: {}; error banner: {}",
        order_edge(&harness, "cart", "probe"),
        harness.query_by_label_contains("Drop the wire").is_some(),
    ));

    // The right-click menu at compact zoom.
    view(&mut harness, &graph, Option_::Today, 0.6);
    let probe_rect = screen(&harness, &probe);
    right_click(&mut harness, probe_rect.center());
    let document = document(&harness);
    let open = app(&mut harness)
        .canvases
        .get(&document)
        .is_some_and(|canvas| canvas.debug_menu_open());
    note(&format!(
        "today right-click on probe at 0.6: Add step menu open: {open}"
    ));
    harness.event(egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    settle(&mut harness);

    // The Inspector's connections for a selected request.
    let header = screen(&harness, &probe).center_top() + vec2(0.0, 12.0);
    click(&mut harness, header);
    note(&format!(
        "today probe selected: Inspector 'Next: connect to a step' {}; 'Body value source' {}; 'Body step' (source step combo) {}",
        has_part(&harness, "connect to a step"),
        has_part(&harness, "Body value source"),
        has_part(&harness, "Body step"),
    ));
    capture(&mut harness, "lay16-today-selected-1600", None);
}

/// One option at one window size: rest, selected, during a wire drag; the
/// heights and what the drag found, noted.
fn option_views(option: Option_, size: egui::Vec2) {
    let (_project, mut harness, graph) = open(size);
    let probe = id_of(&graph, "probe");
    let prefix = format!("lay16-{}", name(option));
    let suffix = suffix(size);
    view(&mut harness, &graph, option, 0.6);
    if size == WIDE {
        note(&format!(
            "option {} heights (world px): login {:.0}, cart {:.0}, probe {:.0}",
            name(option).to_uppercase(),
            mock::world(&id_of(&graph, "login")).unwrap().height(),
            mock::world(&id_of(&graph, "cart")).unwrap().height(),
            mock::world(&probe).unwrap().height(),
        ));
        note(&format!(
            "option {} at rest: output socket body of probe: {}; output socket body of cart: {}",
            name(option).to_uppercase(),
            has(&harness, "Output body of probe"),
            has(&harness, "Output body of cart"),
        ));
    }
    capture(&mut harness, &format!("{prefix}-rest-{suffix}"), None);
    let cards = region(&harness, &graph);
    capture(
        &mut harness,
        &format!("{prefix}-rest-{suffix}-cards"),
        Some(cards),
    );

    // During a wire drag from cart's Response socket.
    let from = socket_center(&harness, "Output body of cart");
    let to = aim(option, screen(&harness, &probe), 0.6);
    press(&mut harness, from);
    move_to(&mut harness, from, to);
    shot_held(&mut harness, &format!("{prefix}-drag-{suffix}"), None);
    let cards = region(&harness, &graph);
    shot_held(
        &mut harness,
        &format!("{prefix}-drag-{suffix}-cards"),
        Some(cards),
    );
    let target = mock::target();
    release(&mut harness, to);
    if size == WIDE {
        note(&format!(
            "option {} data drag cart.body → probe: snapped target {target:?}; probe.body after release: {}; probe.url after release: {}; error banner: {}",
            name(option).to_uppercase(),
            body_binding(&harness, "probe"),
            binding_of(&harness, "probe", "url"),
            has_part(&harness, "Drop the wire"),
        ));
    }

    // Selected.
    let header = screen(&harness, &probe).center_top() + vec2(0.0, 12.0);
    click(&mut harness, header);
    if size == WIDE {
        note(&format!(
            "option {} probe selected: output socket body of probe: {}; status of probe: {}; Inspector 'Next: connect to a step' {}",
            name(option).to_uppercase(),
            has(&harness, "Output body of probe"),
            has(&harness, "Output status of probe"),
            has_part(&harness, "connect to a step"),
        ));
    }
    capture(&mut harness, &format!("{prefix}-selected-{suffix}"), None);
    let cards = region(&harness, &graph);
    capture(
        &mut harness,
        &format!("{prefix}-selected-{suffix}-cards"),
        Some(cards),
    );
    mock::set(Flags::default());
}

#[test]
#[ignore = "design-review view"]
fn review_lay16_option_a() {
    option_views(Option_::A, WIDE);
    option_views(Option_::A, MINIMUM);
}

#[test]
#[ignore = "design-review view"]
fn review_lay16_option_b() {
    option_views(Option_::B, WIDE);
    option_views(Option_::B, MINIMUM);
}

#[test]
#[ignore = "design-review view"]
fn review_lay16_option_c() {
    option_views(Option_::C, WIDE);
    option_views(Option_::C, MINIMUM);
}

#[test]
#[ignore = "design-review view"]
fn review_lay16_option_d() {
    option_views(Option_::D, WIDE);
    option_views(Option_::D, MINIMUM);
}

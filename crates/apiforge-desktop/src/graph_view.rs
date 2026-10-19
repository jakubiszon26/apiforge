//! Fixed card geometry follows the established React editor. Drawing state is
//! separate from the imported document; moving a preview never edits its JSON.
use crate::theme;
use apiforge_core::{
    format::{Binding, Graph, Project},
    model::{Branch, NodeKind, ValueType},
};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use std::collections::HashMap;

const WIDTH: f32 = 260.0;
const HEADER: f32 = 66.0;
const INPUT: f32 = 47.0;
const OUTPUT: f32 = 24.0;
const FOOTER: f32 = 54.0;
struct InputRow {
    key: String,
    label: String,
    value: String,
    linked: bool,
}
struct OutputRow {
    key: String,
    label: String,
    value_type: String,
}
pub struct ViewNode {
    pub id: String,
    name: String,
    kind: NodeKind,
    revision: Option<u32>,
    subtitle: Option<String>,
    inputs: Vec<InputRow>,
    outputs: Vec<OutputRow>,
    position: Pos2,
}
impl ViewNode {
    fn header_height(&self) -> f32 {
        HEADER + if self.subtitle.is_some() { 24.0 } else { 0.0 }
    }
    fn height(&self) -> f32 {
        self.header_height()
            + self.inputs.len() as f32 * INPUT
            + self.outputs.len() as f32 * OUTPUT
            + FOOTER
    }
    fn rect(&self) -> Rect {
        Rect::from_min_size(self.position, egui::vec2(WIDTH, self.height()))
    }
    fn input(&self, key: &str) -> Option<Pos2> {
        if key == "__order" {
            return Some(self.position + egui::vec2(WIDTH / 2.0, 0.0));
        }
        self.inputs
            .iter()
            .position(|row| row.key == key)
            .map(|index| {
                self.position
                    + egui::vec2(
                        0.0,
                        self.header_height() + index as f32 * INPUT + INPUT / 2.0,
                    )
            })
    }
    fn output(&self, key: &str) -> Option<Pos2> {
        if key.starts_with("__") {
            let x = match key {
                "__true" => WIDTH * 0.25,
                "__false" => WIDTH * 0.75,
                _ => WIDTH / 2.0,
            };
            return Some(self.position + egui::vec2(x, self.height()));
        }
        self.outputs
            .iter()
            .position(|row| row.key == key)
            .map(|index| {
                self.position
                    + egui::vec2(
                        WIDTH,
                        self.header_height()
                            + self.inputs.len() as f32 * INPUT
                            + index as f32 * OUTPUT
                            + OUTPUT / 2.0,
                    )
            })
    }
}
struct Wire {
    source: usize,
    port: String,
    target: usize,
    input: String,
    control: bool,
}
pub struct Canvas {
    nodes: Vec<ViewNode>,
    wires: Vec<Wire>,
    pub selected: Option<String>,
    zoom: f32,
    pan: Vec2,
    fit_pending: bool,
    last_size: Vec2,
}
impl Default for Canvas {
    fn default() -> Self {
        Self {
            nodes: vec![],
            wires: vec![],
            selected: None,
            zoom: 1.0,
            pan: Vec2::ZERO,
            fit_pending: true,
            last_size: Vec2::ZERO,
        }
    }
}
impl Canvas {
    pub fn nodes(&self) -> impl Iterator<Item = &ViewNode> {
        self.nodes.iter()
    }
    pub fn request_fit(&mut self) {
        self.fit_pending = true;
    }
    fn bounds(&self) -> Option<Rect> {
        self.nodes.iter().map(ViewNode::rect).reduce(Rect::union)
    }
    fn fit(&mut self, viewport: Rect) {
        if let Some(bounds) = self.bounds() {
            let space = viewport.shrink2(egui::vec2(50.0, 58.0));
            self.zoom = (space.width() / bounds.width())
                .min(space.height() / bounds.height())
                .clamp(0.2, 1.0);
            self.pan = viewport.center().to_vec2() - bounds.center().to_vec2() * self.zoom;
        }
        self.fit_pending = false;
    }
    fn screen(&self, world: Pos2) -> Pos2 {
        (world.to_vec2() * self.zoom + self.pan).to_pos2()
    }
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let (viewport, response) = ui.allocate_exact_size(
            ui.available_size().max(egui::vec2(1.0, 1.0)),
            Sense::click_and_drag(),
        );
        // Resize and inspector/report changes must not leave cards off-screen.
        if (viewport.size() - self.last_size).length() > 1.0 {
            self.fit_pending = true;
            self.last_size = viewport.size();
        }
        // Otherwise zoom and pan remain under user control between frames.
        if self.fit_pending {
            self.fit(viewport);
        }
        let painter = ui.painter().with_clip_rect(viewport);
        painter.rect_filled(viewport, 0.0, theme::BG);
        let mouse = ui.input(|input| input.pointer.hover_pos());
        if response.hovered() {
            let factor = ui.input(|input| input.zoom_delta());
            if factor != 1.0
                && let Some(mouse) = mouse
            {
                self.zoom_at(mouse, factor);
            }
            let scroll = ui.input(|input| input.smooth_scroll_delta);
            self.pan += scroll;
        }
        let started_on_node =
            ui.input(|input| input.pointer.press_origin())
                .is_some_and(|pointer| {
                    self.nodes
                        .iter()
                        .any(|node| self.screen_rect(node.rect()).contains(pointer))
                });
        if response.dragged_by(egui::PointerButton::Middle)
            || response.dragged_by(egui::PointerButton::Secondary)
            || (response.dragged() && !started_on_node)
        {
            self.pan += ui.input(|input| input.pointer.delta());
        }
        let spacing = (22.0 * self.zoom).max(12.0);
        let origin = viewport.min
            + egui::vec2(
                self.pan.x.rem_euclid(spacing),
                self.pan.y.rem_euclid(spacing),
            );
        let mut x = origin.x;
        while x < viewport.right() {
            let mut y = origin.y;
            while y < viewport.bottom() {
                painter.circle_filled(egui::pos2(x, y), 0.7, Color32::from_rgb(214, 210, 203));
                y += spacing;
            }
            x += spacing;
        }
        for wire in &self.wires {
            self.draw_wire(&painter, wire);
        }
        for index in 0..self.nodes.len() {
            let rect = self.screen_rect(self.nodes[index].rect());
            if !rect.intersects(viewport) {
                continue;
            }
            let node_response = ui.interact(
                rect,
                ui.id().with(&self.nodes[index].id),
                Sense::click_and_drag(),
            );
            if node_response.clicked() {
                self.selected = Some(self.nodes[index].id.clone());
            }
            if node_response.dragged() {
                self.nodes[index].position += ui.input(|input| input.pointer.delta()) / self.zoom;
            }
            self.draw_card(&painter, index);
        }
        // Canvas tools have their own hit areas and are painted above the graph.
        let toolbar = Rect::from_min_size(
            viewport.right_top() + egui::vec2(-238.0, 16.0),
            egui::vec2(220.0, 32.0),
        );
        painter.rect_filled(toolbar, 7.0, theme::WHITE);
        painter.rect_stroke(
            toolbar,
            7.0,
            Stroke::new(1.0, theme::BORDER),
            StrokeKind::Inside,
        );
        painter.text(
            toolbar.left_center() + egui::vec2(12.0, 0.0),
            Align2::LEFT_CENTER,
            "Edytor scenariusza",
            FontId::proportional(11.0),
            theme::MUTED,
        );
        let fit_rect = Rect::from_min_size(
            toolbar.right_top() + egui::vec2(-38.0, 0.0),
            egui::vec2(38.0, 32.0),
        );
        if ui
            .interact(fit_rect, ui.id().with("fit"), Sense::click())
            .on_hover_text("Dopasuj widok")
            .clicked()
        {
            self.request_fit();
        }
        painter.text(
            fit_rect.center(),
            Align2::CENTER_CENTER,
            "⛶",
            FontId::proportional(20.0),
            theme::TEXT,
        );
        let tools = Rect::from_min_size(
            viewport.left_bottom() + egui::vec2(16.0, -106.0),
            egui::vec2(32.0, 90.0),
        );
        painter.rect_filled(tools, 6.0, theme::WHITE);
        painter.rect_stroke(
            tools,
            6.0,
            Stroke::new(1.0, theme::BORDER),
            StrokeKind::Inside,
        );
        for (index, text) in ["+", "−", "⛶"].iter().enumerate() {
            let rect = Rect::from_min_size(
                tools.min + egui::vec2(0.0, index as f32 * 30.0),
                egui::vec2(32.0, 30.0),
            );
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                text,
                FontId::proportional(18.0),
                theme::MUTED,
            );
            if ui
                .interact(rect, ui.id().with(("zoom", index)), Sense::click())
                .clicked()
            {
                match index {
                    0 => self.zoom_at(viewport.center(), 1.2),
                    1 => self.zoom_at(viewport.center(), 1.0 / 1.2),
                    _ => self.request_fit(),
                }
            }
        }
        self.minimap(&painter, viewport);
        painter.text(
            viewport.left_top() + egui::vec2(18.0, 24.0),
            Align2::LEFT_CENTER,
            format!(
                "•  EDYTOR SCENARIUSZA     |     {} kroków",
                self.nodes.len()
            ),
            FontId::proportional(10.0),
            theme::MUTED,
        );
        painter.text(viewport.center_bottom()-egui::vec2(0.0,18.0), Align2::CENTER_CENTER, "Kliknij klocek: konfiguracja  ·  Przeciągnij tło: przesuń widok  ·  Ctrl + kółko: zoom", FontId::proportional(10.0), theme::MUTED);
    }
    fn zoom_at(&mut self, point: Pos2, factor: f32) {
        let before = self.zoom;
        self.zoom = (self.zoom * factor).clamp(0.2, 1.6);
        self.pan = point.to_vec2() - (point.to_vec2() - self.pan) * (self.zoom / before);
    }
    fn screen_rect(&self, rect: Rect) -> Rect {
        Rect::from_min_max(self.screen(rect.min), self.screen(rect.max))
    }
    fn draw_wire(&self, painter: &egui::Painter, wire: &Wire) {
        let (Some(start), Some(end)) = (
            self.nodes[wire.source].output(&wire.port),
            self.nodes[wire.target].input(&wire.input),
        ) else {
            return;
        };
        let start = self.screen(start);
        let end = self.screen(end);
        let bend = if wire.control {
            egui::vec2(0.0, 55.0 * self.zoom)
        } else {
            egui::vec2(((end.x - start.x).abs() * 0.45).max(35.0 * self.zoom), 0.0)
        };
        let color = if wire.port == "__false" {
            Color32::from_rgb(181, 133, 96)
        } else if wire.control {
            Color32::from_rgb(130, 158, 140)
        } else {
            Color32::from_rgb(181, 176, 168)
        };
        painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
            [start, start + bend, end - bend, end],
            false,
            Color32::TRANSPARENT,
            Stroke::new(1.4, color),
        ));
        let direction = if wire.control {
            egui::vec2(0.0, 1.0)
        } else {
            egui::vec2(1.0, 0.0)
        };
        let normal = egui::vec2(-direction.y, direction.x);
        painter.add(egui::Shape::convex_polygon(
            vec![
                end,
                end - direction * 6.0 + normal * 3.0,
                end - direction * 6.0 - normal * 3.0,
            ],
            color,
            Stroke::NONE,
        ));
    }
    fn draw_card(&self, painter: &egui::Painter, index: usize) {
        let node = &self.nodes[index];
        let selected = self.selected.as_deref() == Some(&node.id);
        let rect = self.screen_rect(node.rect());
        let z = self.zoom;
        let shadow = rect.translate(egui::vec2(0.0, 3.0));
        painter.rect_filled(shadow, 10.0 * z, Color32::from_black_alpha(12));
        painter.rect_filled(rect, 10.0 * z, theme::WHITE);
        painter.rect_stroke(
            rect,
            10.0 * z,
            Stroke::new(
                if selected { 1.8 } else { 1.0 },
                if selected {
                    theme::ACCENT
                } else {
                    theme::BORDER
                },
            ),
            StrokeKind::Inside,
        );
        let local = |x, y| self.screen(node.position + egui::vec2(x, y));
        let clipped = painter.with_clip_rect(rect.shrink(6.0 * z));
        let color = theme::kind_color(node.kind);
        let icon = Rect::from_min_size(local(14.0, 16.0), egui::vec2(32.0, 32.0) * z);
        clipped.rect_filled(icon, 7.0 * z, color.gamma_multiply(0.12));
        theme::kind_icon(&clipped, icon, node.kind, color);
        let kind = match node.revision {
            Some(revision) => format!("{} · r{}", node.kind.label(), revision),
            None => node.kind.label().into(),
        };
        clipped.text(
            local(56.0, 23.0),
            Align2::LEFT_CENTER,
            kind,
            FontId::proportional(9.0 * z),
            theme::MUTED,
        );
        clipped
            .with_clip_rect(Rect::from_min_max(local(56.0, 30.0), local(247.0, 53.0)))
            .text(
                local(56.0, 41.0),
                Align2::LEFT_CENTER,
                &node.name,
                FontId::new(13.0 * z, theme::bold_family()),
                theme::TEXT,
            );
        if let Some(subtitle) = &node.subtitle {
            clipped.text(
                local(14.0, 73.0),
                Align2::LEFT_CENTER,
                subtitle,
                FontId::monospace(10.0 * z),
                theme::MUTED,
            );
        }
        for (index, row) in node.inputs.iter().enumerate() {
            let y = node.header_height() + index as f32 * INPUT;
            clipped.text(
                local(14.0, y + 10.0),
                Align2::LEFT_CENTER,
                &row.label,
                FontId::proportional(10.0 * z),
                theme::MUTED,
            );
            let field = Rect::from_min_max(local(14.0, y + 20.0), local(246.0, y + 42.0));
            clipped.rect_filled(field, 4.0 * z, Color32::from_rgb(248, 247, 244));
            if !row.linked {
                clipped.rect_stroke(
                    field,
                    4.0 * z,
                    Stroke::new(0.5, theme::BORDER),
                    StrokeKind::Inside,
                );
            }
            clipped.with_clip_rect(field.shrink(4.0 * z)).text(
                local(20.0, y + 31.0),
                Align2::LEFT_CENTER,
                &row.value,
                FontId::proportional(10.5 * z),
                if row.linked { theme::LINK } else { theme::TEXT },
            );
            self.socket(
                painter,
                node.input(&row.key).unwrap_or(node.position),
                false,
            );
        }
        let outputs_y = node.header_height() + node.inputs.len() as f32 * INPUT;
        clipped.line_segment(
            [local(0.0, outputs_y), local(WIDTH, outputs_y)],
            Stroke::new(0.6, theme::BORDER),
        );
        for (index, row) in node.outputs.iter().enumerate() {
            let y = outputs_y + index as f32 * OUTPUT + 12.0;
            clipped.text(
                local(242.0, y),
                Align2::RIGHT_CENTER,
                format!("{}   {}", row.label, row.value_type),
                FontId::proportional(9.0 * z),
                theme::TEXT,
            );
            self.socket(
                painter,
                node.output(&row.key).unwrap_or(node.position),
                false,
            );
        }
        let footer = node.height() - FOOTER;
        clipped.line_segment(
            [local(0.0, footer), local(WIDTH, footer)],
            Stroke::new(0.6, theme::BORDER),
        );
        clipped.text(
            local(14.0, footer + 15.0),
            Align2::LEFT_CENTER,
            "Gotowy do konfiguracji",
            FontId::proportional(9.0 * z),
            theme::MUTED,
        );
        clipped.text(
            local(246.0, footer + 15.0),
            Align2::RIGHT_CENTER,
            "Odczyt",
            FontId::proportional(9.0 * z),
            theme::MUTED,
        );
        if node.kind == NodeKind::If {
            for (x, label, key) in [
                (WIDTH * 0.25, "Tak →", "__true"),
                (WIDTH * 0.75, "Nie →", "__false"),
            ] {
                clipped.text(
                    local(x, node.height() - 12.0),
                    Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(10.0 * z),
                    theme::LINK,
                );
                self.socket(painter, node.output(key).unwrap_or(node.position), true);
            }
        } else {
            clipped.text(
                local(WIDTH / 2.0, node.height() - 12.0),
                Align2::CENTER_CENTER,
                "Dalej →",
                FontId::proportional(10.0 * z),
                theme::MUTED,
            );
            self.socket(
                painter,
                node.output("__order").unwrap_or(node.position),
                true,
            );
        }
        if node.kind != NodeKind::Trigger {
            self.socket(
                painter,
                node.input("__order").unwrap_or(node.position),
                true,
            );
        }
    }
    fn socket(&self, painter: &egui::Painter, world: Pos2, control: bool) {
        let point = self.screen(world);
        let radius = (4.0 * self.zoom).max(2.5);
        if control {
            painter.rect(
                Rect::from_center_size(point, Vec2::splat(radius * 2.0)),
                1.0,
                theme::WHITE,
                Stroke::new(1.0, Color32::from_rgb(172, 165, 154)),
                StrokeKind::Inside,
            );
        } else {
            painter.circle(
                point,
                radius,
                theme::WHITE,
                Stroke::new(1.0, Color32::from_rgb(172, 165, 154)),
            );
        }
    }
    fn minimap(&self, painter: &egui::Painter, viewport: Rect) {
        let Some(bounds) = self.bounds() else {
            return;
        };
        let rect = Rect::from_min_size(
            viewport.right_bottom() - egui::vec2(142.0, 94.0),
            egui::vec2(126.0, 78.0),
        );
        painter.rect(
            rect,
            7.0,
            theme::WHITE,
            Stroke::new(1.0, theme::BORDER),
            StrokeKind::Inside,
        );
        let scale =
            ((rect.width() - 20.0) / bounds.width()).min((rect.height() - 20.0) / bounds.height());
        for node in &self.nodes {
            let min = rect.min + egui::vec2(10.0, 10.0) + (node.position - bounds.min) * scale;
            let fill = if self.selected.as_deref() == Some(&node.id) {
                theme::ACCENT
            } else {
                Color32::from_rgb(224, 219, 211)
            };
            painter.rect_filled(
                Rect::from_min_size(min, node.rect().size() * scale),
                1.0,
                fill,
            );
        }
    }
}
fn value_type(value: ValueType) -> String {
    match value {
        ValueType::Any => "any",
        ValueType::String => "string",
        ValueType::Number => "number",
        ValueType::Boolean => "boolean",
        ValueType::Object => "object",
        ValueType::Array => "array",
    }
    .into()
}
fn binding_label(
    binding: Option<&Binding>,
    name: &str,
    graph: &Graph,
    project: &Project,
) -> String {
    let sensitive = [
        "token",
        "password",
        "secret",
        "authorization",
        "cookie",
        "api-key",
    ]
    .iter()
    .any(|key| name.to_lowercase().contains(key));
    if sensitive {
        return "••••••".into();
    }
    match binding {
        Some(Binding::Reference {
            node_id,
            port,
            path,
        }) => format!(
            "{} → {}{}",
            graph
                .nodes
                .iter()
                .find(|node| &node.id == node_id)
                .map(|node| node.name.as_str())
                .unwrap_or("Wcześniejszy krok"),
            port,
            if path.is_empty() {
                String::new()
            } else {
                format!(".{path}")
            }
        ),
        Some(Binding::Parameter { name }) => format!("Argument → {name}"),
        Some(Binding::Environment { name }) => format!("Środowisko → {name}"),
        Some(Binding::Expression { .. }) => "Operator · zagnieżdżone bloki".into(),
        Some(Binding::Literal { value }) => {
            let mut label = if let Some(object) = value.as_object() {
                format!("{{ {} pól }}", object.len())
            } else if let Some(array) = value.as_array() {
                format!("[ {} elementów ]", array.len())
            } else {
                value
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| value.to_string())
            };
            for name in &project.environment.secret_names {
                if let Some(value) = project
                    .environment
                    .variables
                    .get(name)
                    .filter(|v| !v.is_empty())
                {
                    label = label.replace(value, "••••••");
                }
            }
            label
        }
        None => "Opcjonalne".into(),
    }
}
pub fn project_graph(graph: &Graph, project: &Project) -> Canvas {
    let mut result = Canvas::default();
    let mut ids = HashMap::new();
    for node in &graph.nodes {
        let component = project.components.iter().find(|def| {
            Some(&def.id) == node.config.component_id.as_ref()
                && Some(def.revision) == node.config.revision
        });
        let ports: Vec<(String, String)> = match node.kind {
            NodeKind::Component => component
                .map(|def| {
                    def.inputs
                        .iter()
                        .map(|port| {
                            (
                                port.name.clone(),
                                port.label.clone().unwrap_or_else(|| port.name.clone()),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default(),
            NodeKind::Assert | NodeKind::If | NodeKind::Condition => vec![
                ("actual".into(), "Wartość otrzymana".into()),
                ("expected".into(), "Wartość oczekiwana".into()),
            ],
            _ => node
                .inputs
                .keys()
                .map(|key| (key.clone(), key.clone()))
                .collect(),
        };
        let inputs = ports
            .into_iter()
            .map(|(key, label)| InputRow {
                value: binding_label(node.inputs.get(&key), &key, graph, project),
                linked: node
                    .inputs
                    .get(&key)
                    .is_some_and(|b| !matches!(b, Binding::Literal { .. })),
                key,
                label,
            })
            .collect();
        let outputs: Vec<OutputRow> = match node.kind {
            NodeKind::Trigger => vec![],
            NodeKind::Request => [
                ("body", "Odpowiedź", "any"),
                ("status", "Status HTTP", "number"),
                ("headers", "Nagłówki", "object"),
                ("duration", "Czas", "number"),
            ]
            .into_iter()
            .map(|(key, label, value_type)| OutputRow {
                key: key.into(),
                label: label.into(),
                value_type: value_type.into(),
            })
            .collect(),
            NodeKind::Component => component
                .map(|def| {
                    def.outputs
                        .iter()
                        .map(|port| OutputRow {
                            key: port.name.clone(),
                            label: port.label.clone().unwrap_or_else(|| port.name.clone()),
                            value_type: value_type(port.value_type),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            NodeKind::Assert | NodeKind::If | NodeKind::Condition => {
                [("value", "any"), ("passed", "boolean")]
                    .into_iter()
                    .map(|(key, kind)| OutputRow {
                        key: key.into(),
                        label: key.into(),
                        value_type: kind.into(),
                    })
                    .collect()
            }
            _ => vec![OutputRow {
                key: "value".into(),
                label: "value".into(),
                value_type: "any".into(),
            }],
        };
        let subtitle = if node.kind == NodeKind::Request {
            Some(format!(
                "{}   {}",
                match node.config.method {
                    Some(apiforge_core::model::HttpMethod::Post) => "POST",
                    Some(apiforge_core::model::HttpMethod::Put) => "PUT",
                    Some(apiforge_core::model::HttpMethod::Delete) => "DELETE",
                    Some(apiforge_core::model::HttpMethod::Patch) => "PATCH",
                    Some(apiforge_core::model::HttpMethod::Head) => "HEAD",
                    Some(apiforge_core::model::HttpMethod::Options) => "OPTIONS",
                    _ => "GET",
                },
                node.config
                    .url
                    .as_deref()
                    .unwrap_or("/endpoint")
                    .split('?')
                    .next()
                    .unwrap_or("/endpoint")
                    .rsplit('@')
                    .next()
                    .unwrap_or("/endpoint")
            ))
        } else if node.kind == NodeKind::Formula {
            Some(
                match node.config.operation {
                    Some(apiforge_core::model::Operator::Multiply) => "A × B",
                    _ => "Operator",
                }
                .into(),
            )
        } else {
            None
        };
        ids.insert(node.id.clone(), result.nodes.len());
        result.nodes.push(ViewNode {
            id: node.id.clone(),
            name: node.name.clone(),
            kind: node.kind,
            revision: node.config.revision,
            subtitle,
            inputs,
            outputs,
            position: egui::pos2(
                node.position.x.clamp(-1_000_000.0, 1_000_000.0) as f32,
                node.position.y.clamp(-1_000_000.0, 1_000_000.0) as f32,
            ),
        });
    }
    for edge in &graph.edges {
        if let (Some(&source), Some(&target)) = (ids.get(&edge.source), ids.get(&edge.target)) {
            result.wires.push(Wire {
                source,
                target,
                port: match edge.branch {
                    Some(Branch::True) => "__true",
                    Some(Branch::False) => "__false",
                    None => "__order",
                }
                .into(),
                input: "__order".into(),
                control: true,
            });
        }
    }
    for node in &graph.nodes {
        for (name, binding) in &node.inputs {
            let mut references = Vec::new();
            binding.references(&mut references);
            for (source, port) in references {
                if let (Some(&source), Some(&target)) = (ids.get(source), ids.get(&node.id)) {
                    result.wires.push(Wire {
                        source,
                        target,
                        port: port.into(),
                        input: name.clone(),
                        control: false,
                    });
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use apiforge_core::{ImportLimits, import_document};
    fn fixture() -> apiforge_core::ValidatedDocument {
        import_document(
            include_bytes!("../../../fixtures/compatibility/projects/workflow.json"),
            &ImportLimits::default(),
        )
        .expect("fixture")
    }
    #[test]
    fn control_branches_and_nested_references_have_valid_geometry() {
        let doc = fixture();
        let project = &doc.workspace().projects[0];
        let graph = project_graph(&project.scenarios[0].graph, project);
        assert_eq!(graph.nodes.len(), project.scenarios[0].graph.nodes.len());
        for wire in &graph.wires {
            assert!(graph.nodes[wire.source].output(&wire.port).is_some());
            assert!(graph.nodes[wire.target].input(&wire.input).is_some());
        }
        let if_node = graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::If)
            .expect("IF");
        assert_ne!(if_node.output("__true"), if_node.output("__false"));
    }
    #[test]
    fn fit_keeps_every_card_inside_the_visible_canvas() {
        let doc = fixture();
        let project = &doc.workspace().projects[0];
        let mut graph = project_graph(&project.scenarios[0].graph, project);
        let viewport = Rect::from_min_size(egui::pos2(235.0, 212.0), egui::vec2(1050.0, 500.0));
        graph.fit(viewport);
        for node in &graph.nodes {
            assert!(viewport.contains_rect(graph.screen_rect(node.rect())));
        }
    }
    #[test]
    fn clicking_a_card_selects_the_matching_document_node() {
        let doc = fixture();
        let project = &doc.workspace().projects[0];
        let mut graph = project_graph(&project.scenarios[0].graph, project);
        let context = egui::Context::default();
        theme::apply(&context);
        let screen = Rect::from_min_size(Pos2::ZERO, egui::vec2(1100.0, 700.0));
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |ui| graph.show(ui),
        );
        output.textures_delta.clear();
        let point = graph.screen_rect(graph.nodes[0].rect()).center();
        for pressed in [true, false] {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                events: vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
                ..Default::default()
            };
            let mut output = context.run_ui(input, |ui| graph.show(ui));
            output.textures_delta.clear();
        }
        assert_eq!(graph.selected.as_deref(), Some("flow-start"));
    }
}

//! LAY-25 mock-up (throwaway, `mock/lay-25-variables`): what the board would
//! show when it proposes saving a far value in a variable. Nothing here
//! edits a scenario: the views draw the proposal on fixtures written before
//! and after the edit.
use super::{Camera, readers};
use crate::theme;
use eframe::egui::{self, Align2, FontId, Painter, Rect, RichText, Stroke, StrokeKind, pos2, vec2};

/// What the mock-up draws on top of the board.
#[derive(Clone, Debug, Default)]
pub struct Mock {
    /// The choice at a reader's input the moment a far value is connected:
    /// the reader's node id, the variable's proposed name, the value it
    /// would hold and the number of other steps that read the value.
    pub offer: Option<(String, String, String, usize)>,
    /// `+ var` under the count of a step's output read by many steps.
    pub suggest: bool,
    /// The source whose suggestion is open: its fields, each one click.
    pub open: Option<String>,
}

/// The variable a value would be saved as: its last field, after the step
/// where the field alone says nothing (`cart.id` → `cartId`).
pub fn proposed_name(chip: &str) -> String {
    let (step, field) = chip.split_once('.').unwrap_or(("", chip));
    if matches!(field, "id" | "value" | "body" | "status") && !step.is_empty() {
        let mut name = step.to_owned();
        let mut letters = field.chars();
        if let Some(first) = letters.next() {
            name.extend(first.to_uppercase());
            name.push_str(letters.as_str());
        }
        name
    } else {
        field.to_owned()
    }
}

/// The suggestion under a count: an action, so in the accent.
pub fn paint_suggestion(painter: &Painter, count: &readers::Laid) -> Rect {
    let font = FontId::proportional(count.font);
    let galley = painter.layout_no_wrap("+ var".to_owned(), font, theme::ACCENT);
    let size = galley.size() + vec2(8.0, 3.0);
    let rect = Rect::from_min_size(pos2(count.rect.left(), count.rect.bottom() + 3.0), size);
    painter.rect(
        rect,
        rect.height() / 2.0,
        theme::WHITE,
        Stroke::new(1.0, theme::ACCENT),
        StrokeKind::Inside,
    );
    painter.galley(rect.min + vec2(4.0, 1.5), galley, theme::ACCENT);
    rect
}

/// The menu of a suggestion: one row per field three or more steps read,
/// as `(variable, value, readers)`.
pub fn show_suggestions(ctx: &egui::Context, at: egui::Pos2, rows: &[(String, String, usize)]) {
    egui::Area::new(egui::Id::new("lay25-suggestions"))
        .order(egui::Order::Foreground)
        .fixed_pos(at + vec2(0.0, 4.0))
        .constrain(true)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .fill(theme::WHITE)
                .stroke(Stroke::new(1.0, theme::BORDER))
                .corner_radius(10)
                .inner_margin(10)
                .show(ui, |ui| {
                    ui.set_width(400.0);
                    ui.label(
                        RichText::new("SAVE AS VARIABLE")
                            .size(10.0)
                            .color(theme::MUTED),
                    );
                    for (index, (name, value, readers)) in rows.iter().enumerate() {
                        ui.add_space(2.0);
                        let mut frame = egui::Frame::new().corner_radius(6).inner_margin(8);
                        if index == 0 {
                            frame = frame
                                .fill(theme::ACCENT_SOFT)
                                .stroke(Stroke::new(1.0, theme::ACCENT));
                        }
                        frame.show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!("var {name} = {value}"))
                                        .monospace()
                                        .size(12.0)
                                        .color(theme::TEXT),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            RichText::new(format!("{readers} steps read it"))
                                                .size(11.0)
                                                .color(theme::MUTED),
                                        );
                                    },
                                );
                            });
                        });
                    }
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new("The steps that read the value read the variable. One undo.")
                            .size(11.0)
                            .color(theme::MUTED),
                    );
                });
        });
}

/// The choice shown where a wire from a far value was dropped.
pub fn show_offer(
    ctx: &egui::Context,
    camera: Camera,
    anchor: egui::Pos2,
    name: &str,
    value: &str,
    others: usize,
) {
    let at = camera.to_screen(anchor) + vec2(-8.0, 14.0);
    egui::Area::new(egui::Id::new("lay25-offer"))
        .order(egui::Order::Foreground)
        .pivot(Align2::RIGHT_TOP)
        .fixed_pos(at)
        .constrain(true)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .fill(theme::WHITE)
                .stroke(Stroke::new(1.0, theme::BORDER))
                .corner_radius(10)
                .inner_margin(10)
                .show(ui, |ui| {
                    ui.set_width(312.0);
                    ui.label(
                        RichText::new(format!("{} IS FAR FROM THIS STEP", value.to_uppercase()))
                            .size(10.0)
                            .color(theme::MUTED),
                    );
                    ui.add_space(4.0);
                    egui::Frame::new()
                        .fill(theme::ACCENT_SOFT)
                        .stroke(Stroke::new(1.0, theme::ACCENT))
                        .corner_radius(6)
                        .inner_margin(8)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!("Read via variable {name}"))
                                        .family(theme::bold_family())
                                        .size(13.0)
                                        .color(theme::TEXT),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            RichText::new("Enter").size(10.0).color(theme::MUTED),
                                        );
                                    },
                                );
                            });
                            ui.label(
                                RichText::new(format!("var {name} = {value}"))
                                    .monospace()
                                    .size(11.0)
                                    .color(theme::TEXT),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "The {others} steps that read it read {name} too. One undo."
                                ))
                                .size(11.0)
                                .color(theme::MUTED),
                            );
                        });
                    ui.add_space(2.0);
                    egui::Frame::new().inner_margin(8).show(ui, |ui| {
                        ui.label(
                            RichText::new(format!("Wire directly to {value}"))
                                .size(13.0)
                                .color(theme::TEXT),
                        );
                    });
                });
        });
}

/// The values the scenario-wide action would save: each field of a
/// request's or a component call's output that three or more steps read,
/// one of them far, and no variable holds, as `(variable, value, readers)`
/// in document order. Far is counted in the engine's order: the reader's
/// step is two or more steps after the source's.
pub fn shared_values(graph: &apiforge_core::format::Graph) -> Vec<(String, String, usize)> {
    use apiforge_core::model::NodeKind;
    use std::collections::{BTreeSet, HashMap};
    let kind = |id: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.id == id)
            .map(|n| n.kind)
    };
    let mut step = 0usize;
    let mut step_of: HashMap<&str, usize> = HashMap::new();
    for id in apiforge_core::graph::sequence(graph) {
        if matches!(
            kind(id),
            Some(NodeKind::Request | NodeKind::Component | NodeKind::If)
        ) {
            step += 1;
        }
        step_of.insert(id, step);
    }
    let mut rows: Vec<(String, String, BTreeSet<&str>, bool)> = Vec::new();
    for node in &graph.nodes {
        for reference in node
            .inputs
            .values()
            .flat_map(apiforge_core::format::Binding::references)
        {
            let Some(source) = graph.nodes.iter().find(|source| {
                source.id == reference.node_id
                    && matches!(source.kind, NodeKind::Request | NodeKind::Component)
            }) else {
                continue;
            };
            let far = step_of[node.id.as_str()] >= step_of[source.id.as_str()] + 2;
            let field = reference.path.rsplit('.').next().unwrap_or_default();
            let value = if reference.path.is_empty() {
                format!("{}.{}", source.handle, reference.port)
            } else if reference.path.starts_with(reference.port) {
                format!("{}.{}", source.handle, reference.path)
            } else {
                format!("{}.{}.{}", source.handle, reference.port, reference.path)
            };
            let field = if field.is_empty() {
                reference.port
            } else {
                field
            };
            match rows.iter_mut().find(|row| row.1 == value) {
                Some(row) => {
                    row.2.insert(node.id.as_str());
                    row.3 |= far;
                }
                None => rows.push((
                    proposed_name(&format!("{}.{field}", source.handle)),
                    value,
                    BTreeSet::from([node.id.as_str()]),
                    far,
                )),
            }
        }
    }
    rows.into_iter()
        .filter(|row| row.2.len() >= readers::MIN_READERS && row.3)
        .map(|(name, value, readers, _)| (name, value, readers.len()))
        .collect()
}

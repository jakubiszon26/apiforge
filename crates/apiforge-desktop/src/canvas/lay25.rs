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

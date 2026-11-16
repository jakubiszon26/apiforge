//! Saving a far value in a variable, proposed by the board (LAY-25): the
//! choice at the input where a far value was dropped, and the menu of a
//! port's `+ var` mark. Both only say what to do; the edits are the
//! document's (`Command::SaveValues`).
use super::Command;
use crate::theme;
use apiforge_core::edit::SharedValue;
use eframe::egui::{self, Align2, FontId, Key, Pos2, Rect, RichText, Sense, Stroke, vec2};

/// What a popup asks of the board this frame.
pub enum Outcome {
    Open,
    Close,
    Run(Command),
}

/// The choice at an input where a far value was dropped: read it through a
/// variable (first, under Enter) or wire it directly, as before.
pub struct Offer {
    /// The input's socket, on the board.
    pub anchor: Pos2,
    pub proposal: SharedValue,
    /// The connection as it is made without a variable.
    pub direct: Command,
    /// The first row takes the keyboard focus when the choice opens.
    pub focus: bool,
}

/// The first row's name: *Read via variable cartId*, or *Read cartId* where
/// a variable already holds the value.
pub fn first_row(proposal: &SharedValue) -> String {
    if proposal.held_by.is_some() {
        format!("Read {}", proposal.name)
    } else {
        format!("Read via variable {}", proposal.name)
    }
}

pub fn second_row(proposal: &SharedValue) -> String {
    format!("Wire directly to {}", proposal.text)
}

/// Text inside a row, painted: the row carries the name for assistive
/// technology, so its parts are not read out a second time.
fn painted(ui: &mut egui::Ui, text: impl Into<String>, font: FontId, color: egui::Color32) {
    let galley = ui
        .painter()
        .layout(text.into(), font, color, ui.available_width().max(1.0));
    let (_, rect) = ui.allocate_space(galley.size());
    ui.painter().galley(rect.min, galley, color);
}

/// A row of a popup: a frame filled while it is the proposal, clickable and
/// named `name` for the keyboard and assistive technology.
fn row(
    ui: &mut egui::Ui,
    id: egui::Id,
    name: &str,
    accent: bool,
    enabled: bool,
    content: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    let mut frame = egui::Frame::new().corner_radius(6).inner_margin(8);
    if accent {
        frame = frame
            .fill(theme::ACCENT_SOFT)
            .stroke(Stroke::new(1.0, theme::ACCENT));
    }
    let shown = frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        content(ui);
    });
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let response = ui.interact(shown.response.rect, id, sense);
    if enabled && (response.hovered() || response.has_focus()) {
        ui.painter().rect_stroke(
            shown.response.rect,
            6.0,
            Stroke::new(1.5, theme::ACCENT),
            egui::StrokeKind::Inside,
        );
    }
    let name = name.to_owned();
    response
        .widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &name));
    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

/// Closes on Esc, and on a click outside `area` that did not open it.
fn closing(ctx: &egui::Context, area: &egui::Response, opening: bool) -> bool {
    if ctx.input(|input| input.key_pressed(Key::Escape)) {
        return true;
    }
    !opening
        && ctx.input(|input| input.pointer.primary_clicked())
        && ctx
            .input(|input| input.pointer.press_origin())
            .is_some_and(|origin| !area.rect.contains(origin))
}

impl Offer {
    /// The choice left of the input, inside `board`.
    pub fn show(&mut self, ctx: &egui::Context, at: Pos2, board: Rect) -> (Outcome, Rect) {
        let opening = self.focus;
        let mut outcome = Outcome::Open;
        let proposal = &self.proposal;
        let area = egui::Area::new(egui::Id::new("lay25-offer"))
            .order(egui::Order::Foreground)
            .pivot(Align2::RIGHT_TOP)
            .fixed_pos(at + vec2(-8.0, 14.0))
            .constrain_to(board)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .fill(theme::WHITE)
                    .stroke(Stroke::new(1.0, theme::BORDER))
                    .corner_radius(10)
                    .inner_margin(10)
                    .show(ui, |ui| {
                        ui.set_width(312.0_f32.min(board.width() - 40.0));
                        ui.label(
                            RichText::new(format!("{} is far from this step", proposal.text))
                                .size(10.0)
                                .color(theme::MUTED),
                        );
                        ui.add_space(4.0);
                        let first = first_row(proposal);
                        let via = row(ui, ui.id().with("via"), &first, true, true, |ui| {
                            ui.horizontal(|ui| {
                                painted(
                                    ui,
                                    &first,
                                    FontId::new(13.0, theme::bold_family()),
                                    theme::TEXT,
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        painted(
                                            ui,
                                            "Enter",
                                            FontId::proportional(10.0),
                                            theme::MUTED,
                                        );
                                    },
                                );
                            });
                            if proposal.held_by.is_none() {
                                painted(ui, proposal.line(), FontId::monospace(11.0), theme::TEXT);
                            }
                            painted(
                                ui,
                                format!(
                                    "The steps that read {} read {} too. One undo.",
                                    proposal.text, proposal.name
                                ),
                                FontId::proportional(11.0),
                                theme::MUTED,
                            );
                        });
                        ui.add_space(2.0);
                        let second = second_row(proposal);
                        let direct = row(ui, ui.id().with("direct"), &second, false, true, |ui| {
                            painted(ui, &second, FontId::proportional(13.0), theme::TEXT);
                        });
                        if self.focus {
                            via.request_focus();
                            self.focus = false;
                        }
                        let enter = ui.input(|input| input.key_pressed(Key::Enter));
                        if direct.clicked() {
                            outcome = Outcome::Run(self.direct.clone());
                        } else if via.clicked() || (enter && !direct.has_focus()) {
                            outcome = Outcome::Run(Command::SaveValues {
                                values: vec![proposal.value.clone()],
                                first: Some(Box::new(self.direct.clone())),
                            });
                        }
                    });
            });
        if matches!(outcome, Outcome::Open) && closing(ctx, &area.response, opening) {
            outcome = Outcome::Close;
        }
        (outcome, area.response.rect)
    }
}

/// The menu of a port's `+ var`: one row per value of the port worth a
/// variable, each with the line it writes and the number of steps.
pub struct PortMenu {
    pub source: String,
    pub port: String,
    /// Under the mark, on screen.
    pub at: Pos2,
    pub opening: bool,
}

/// The accessible name of a `+ var` mark.
pub fn mark_name(step: &str) -> String {
    format!("Save a value of {step} as a variable")
}

/// A row of the port's menu: `var cartId = cart.id, 9 steps read it`.
pub fn port_row(value: &SharedValue) -> String {
    let line = if value.held_by.is_some() {
        format!("Read {} for {}", value.name, value.text)
    } else {
        value.line()
    };
    let steps = if value.readers == 1 {
        "step reads"
    } else {
        "steps read"
    };
    format!("{line}, {} {steps} it", value.readers)
}

impl PortMenu {
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        values: &[&SharedValue],
        editable: bool,
    ) -> Outcome {
        let opening = std::mem::take(&mut self.opening);
        let mut outcome = Outcome::Open;
        let area = egui::Area::new(egui::Id::new("lay25-port-menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(self.at + vec2(0.0, 4.0))
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
                        for (index, value) in values.iter().enumerate() {
                            ui.add_space(2.0);
                            let name = port_row(value);
                            let response = row(
                                ui,
                                ui.id().with(("row", index)),
                                &name,
                                index == 0,
                                editable,
                                |ui| {
                                    ui.horizontal(|ui| {
                                        let line = if value.held_by.is_some() {
                                            format!("Read {} for {}", value.name, value.text)
                                        } else {
                                            value.line()
                                        };
                                        painted(ui, line, FontId::monospace(12.0), theme::TEXT);
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                painted(
                                                    ui,
                                                    format!("{} steps read it", value.readers),
                                                    FontId::proportional(11.0),
                                                    theme::MUTED,
                                                );
                                            },
                                        );
                                    });
                                },
                            );
                            if response.clicked() {
                                outcome = Outcome::Run(Command::SaveValues {
                                    values: vec![value.value.clone()],
                                    first: None,
                                });
                            }
                        }
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new(
                                "The steps that read the value read the variable. One undo.",
                            )
                            .size(11.0)
                            .color(theme::MUTED),
                        );
                    });
            });
        if matches!(outcome, Outcome::Open) && closing(ctx, &area.response, opening) {
            outcome = Outcome::Close;
        }
        outcome
    }
}

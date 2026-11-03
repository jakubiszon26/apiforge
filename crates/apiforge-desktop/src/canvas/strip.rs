//! A component's INPUTS strip (UX-31, COMP-00 Q1 = C): one pill per input
//! under the board's header row, and *+ Add input*. Selecting a pill rings
//! the steps that read the input. The strip is drawn from the component's
//! declared inputs and stores nothing.

use crate::icons::Icon;
use crate::{theme, widgets};
use apiforge_core::format::{Contract, OptionalJson, Port};
use eframe::egui::{
    self, Align, FontId, Rect, Response, Sense, Stroke, StrokeKind, Vec2, pos2, vec2,
};

/// The strip's height under the header row.
pub const HEIGHT: f32 = 40.0;
const PILL_HEIGHT: f32 = 26.0;
const PILL_PADDING: f32 = 12.0;
const PILL_GAP: f32 = 8.0;
/// Between the name, with its required mark, and the detail.
const DETAIL_GAP: f32 = 8.0;
/// The room *+ Add input* takes before it has been drawn once.
const ADD_WIDTH: f32 = 110.0;
const ADD_HOVER: &str = "Coming with UX-32";

/// One input as its pill and the Inspector show it.
#[derive(Clone, Debug, PartialEq)]
pub struct Pill {
    pub name: String,
    pub required: bool,
    pub value_type: String,
    /// The contract's name, or its identifier when the project has none of
    /// that identifier.
    pub contract: Option<String>,
    /// The default as written in code: `1`, `"EUR"`.
    pub default: Option<String>,
    /// The input's display label, when it has one.
    pub label: Option<String>,
    /// Whether a block calling the component lets its value be edited.
    pub exposed: bool,
}

impl Pill {
    pub fn of(port: &Port, contracts: &[Contract]) -> Self {
        let default = match &port.default_value {
            OptionalJson::Present(value) => Some(value.to_string()),
            OptionalJson::Missing => None,
        };
        Self {
            name: port.name.clone(),
            // An input with a default is optional unless declared required
            // (KTLO-32).
            required: port.required.unwrap_or(default.is_none()),
            value_type: port.value_type.as_str().to_owned(),
            contract: port.contract_id.as_ref().map(|id| {
                contracts
                    .iter()
                    .find(|contract| &contract.id == id)
                    .map_or_else(|| id.clone(), |contract| contract.name.clone())
            }),
            default,
            label: port.label.clone(),
            exposed: port.exposed.unwrap_or(true),
        }
    }

    /// `number = 1`, `object · Product`: the type, then the contract or the
    /// default.
    pub fn detail(&self) -> String {
        let mut detail = self.value_type.clone();
        if let Some(contract) = &self.contract {
            detail = format!("{detail} · {contract}");
        }
        if let Some(default) = &self.default {
            detail = format!("{detail} = {default}");
        }
        detail
    }

    /// What the pill reads, which is also its accessible name:
    /// `quantity * number = 1`.
    pub fn text(&self) -> String {
        format!(
            "{}{} {}",
            self.name,
            if self.required { " *" } else { "" },
            self.detail()
        )
    }

    fn hover(&self) -> String {
        let mut lines = Vec::new();
        if let Some(label) = &self.label {
            lines.push(label.clone());
        }
        lines.push(format!(
            "{} input, read as param.{}",
            if self.required {
                "A required"
            } else {
                "An optional"
            },
            self.name
        ));
        if !self.exposed {
            lines.push("Not editable on a block that calls the component".to_owned());
        }
        lines.push("Click to ring the steps that read it".to_owned());
        lines.join("\n")
    }
}

/// What a click in the strip asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// Select this input, or clear the selection when it is `None`.
    Select(Option<String>),
}

fn name_font() -> FontId {
    FontId::new(theme::FONT_LABEL, theme::bold_family())
}

fn detail_font() -> FontId {
    FontId::proportional(theme::FONT_META)
}

fn text_width(ui: &egui::Ui, text: &str, font: FontId) -> f32 {
    ui.painter()
        .layout_no_wrap(text.to_owned(), font, theme::TEXT)
        .size()
        .x
}

fn pill_width(ui: &egui::Ui, pill: &Pill) -> f32 {
    let star = if pill.required {
        text_width(ui, " *", name_font())
    } else {
        0.0
    };
    PILL_PADDING
        + text_width(ui, &pill.name, name_font())
        + star
        + DETAIL_GAP
        + text_width(ui, &pill.detail(), detail_font())
        + PILL_PADDING
}

/// Draws the strip into `rect`, above the board, and returns what a click
/// asked for. The pills scroll sideways when they do not fit, so
/// *+ Add input* always stays whole at their end.
pub fn show(ui: &egui::Ui, rect: Rect, pills: &[Pill], selected: Option<&str>) -> Option<Outcome> {
    let mut outcome = None;
    let id = ui.id().with("board-inputs-strip");
    egui::Area::new(id)
        .enabled(ui.is_enabled())
        .fixed_pos(rect.min)
        .constrain(false)
        .fade_in(false)
        .show(ui.ctx(), |ui| {
            ui.set_min_size(rect.size());
            ui.set_max_size(rect.size());
            let painter = ui.painter();
            painter.rect_filled(rect, 0.0, theme::WHITE);
            painter.line_segment(
                [rect.left_bottom(), rect.right_bottom()],
                Stroke::new(1.0, theme::BORDER),
            );
            let inner = Rect::from_min_max(
                pos2(
                    rect.left() + super::layout::OVERLAY_PADDING_LEFT,
                    rect.top(),
                ),
                pos2(
                    rect.right() - super::layout::OVERLAY_PADDING_RIGHT,
                    rect.bottom(),
                ),
            );
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(inner)
                    .layout(egui::Layout::left_to_right(Align::Center)),
                |ui| {
                    ui.spacing_mut().item_spacing.x = PILL_GAP;
                    widgets::caption(ui, "INPUTS");
                    ui.add_space(theme::SPACE_SM);
                    let add_key = id.with("add-width");
                    let add_width = ui
                        .ctx()
                        .data(|data| data.get_temp::<f32>(add_key))
                        .unwrap_or(ADD_WIDTH);
                    let widths: Vec<f32> = pills.iter().map(|pill| pill_width(ui, pill)).collect();
                    let total = widths.iter().sum::<f32>()
                        + PILL_GAP * widths.len().saturating_sub(1) as f32;
                    let room = (ui.available_width() - add_width - 2.0 * PILL_GAP).max(0.0);
                    // The selected pill is scrolled into view when it is
                    // selected and whenever the strip's room changes, as when
                    // the Inspector opens beside the board.
                    let shown_key = id.with("shown");
                    let shown = (selected.map(str::to_owned), room.round() as i32);
                    let reveal = ui
                        .ctx()
                        .data(|data| data.get_temp::<(Option<String>, i32)>(shown_key))
                        .as_ref()
                        != Some(&shown);
                    if reveal {
                        ui.ctx().data_mut(|data| data.insert_temp(shown_key, shown));
                    }
                    // Where the selected pill starts and ends among the pills:
                    // the offset that shows it whole at the right edge.
                    let reveal_offset = reveal
                        .then(|| {
                            let index = pills
                                .iter()
                                .position(|pill| selected == Some(pill.name.as_str()))?;
                            let left: f32 =
                                widths[..index].iter().map(|width| width + PILL_GAP).sum();
                            let right = left + widths[index];
                            Some((right - room).max(0.0).min(left))
                        })
                        .flatten();
                    let mut row = |ui: &mut egui::Ui| {
                        for (pill, width) in pills.iter().zip(&widths) {
                            let on = selected == Some(pill.name.as_str());
                            if self::pill(ui, pill, *width, on).clicked() {
                                outcome = Some(Outcome::Select((!on).then(|| pill.name.clone())));
                            }
                        }
                    };
                    if total <= room {
                        row(ui);
                    } else {
                        let mut area = egui::ScrollArea::horizontal()
                            .id_salt("board-inputs-pills")
                            .max_width(room);
                        if let Some(offset) = reveal_offset {
                            area = area.horizontal_scroll_offset(offset);
                        }
                        area.show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = PILL_GAP;
                                row(ui);
                            });
                        });
                    }
                    ui.add_space(4.0);
                    let add = ui
                        .add_enabled(false, widgets::action(Icon::Plus, "Add input").quiet())
                        .on_disabled_hover_text(ADD_HOVER);
                    let width = add.rect.width();
                    if ui.ctx().data(|data| data.get_temp::<f32>(add_key)) != Some(width) {
                        ui.ctx().data_mut(|data| data.insert_temp(add_key, width));
                        ui.ctx().request_repaint();
                    }
                },
            );
        });
    outcome
}

/// One input's pill: its name, a red required mark and the muted detail;
/// selected, it takes the accent ring of the steps it rings.
fn pill(ui: &mut egui::Ui, pill: &Pill, width: f32, selected: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(width, PILL_HEIGHT), Sense::click());
    let text = pill.text();
    let enabled = ui.is_enabled();
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, selected, &text)
    });
    let response = response.on_hover_text(pill.hover());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let (fill, stroke) = if selected {
            (theme::ACCENT_SOFT, Stroke::new(1.7, theme::ACCENT))
        } else if response.hovered() {
            (
                theme::LINK.gamma_multiply(0.12),
                Stroke::new(1.0, theme::LINK.gamma_multiply(0.75)),
            )
        } else {
            (
                theme::LINK.gamma_multiply(0.08),
                Stroke::new(1.0, theme::LINK.gamma_multiply(0.45)),
            )
        };
        let radius = PILL_HEIGHT / 2.0;
        painter.rect(rect, radius, fill, stroke, StrokeKind::Inside);
        if response.has_focus() {
            painter.rect_stroke(
                rect.expand(2.0),
                radius + 2.0,
                ui.visuals().selection.stroke,
                StrokeKind::Outside,
            );
        }
        // Each part is drawn at `x` and returns where the next one starts.
        let put = |x: f32, text: &str, font: FontId, color| {
            let galley = painter.layout_no_wrap(text.to_owned(), font, color);
            let size: Vec2 = galley.size();
            painter.galley(pos2(x, rect.center().y - size.y / 2.0), galley, color);
            x + size.x
        };
        let mut x = put(
            rect.left() + PILL_PADDING,
            &pill.name,
            name_font(),
            theme::TEXT,
        );
        if pill.required {
            x = put(x, " *", name_font(), theme::DANGER);
        }
        put(x + DETAIL_GAP, &pill.detail(), detail_font(), theme::MUTED);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use apiforge_core::model::ValueType;
    use serde_json::json;

    fn contracts() -> Vec<Contract> {
        vec![Contract {
            id: "product-v1".into(),
            name: "Product".into(),
            sample: json!({}),
        }]
    }

    #[test]
    fn a_pill_reads_name_required_mark_type_and_contract_or_default() {
        let product = Port {
            contract_id: Some("product-v1".into()),
            ..Port::new("product", ValueType::Object)
        }
        .required(true);
        assert_eq!(
            Pill::of(&product, &contracts()).text(),
            "product * object · Product"
        );
        let quantity = Port {
            default_value: OptionalJson::Present(json!(1)),
            ..Port::new("quantity", ValueType::Number)
        }
        .required(true);
        assert_eq!(
            Pill::of(&quantity, &contracts()).text(),
            "quantity * number = 1"
        );
        // A default makes an undeclared input optional (KTLO-32).
        let currency = Port {
            default_value: OptionalJson::Present(json!("EUR")),
            ..Port::new("currency", ValueType::String)
        };
        assert_eq!(
            Pill::of(&currency, &contracts()).text(),
            "currency string = \"EUR\""
        );
        // An unknown contract is named by its identifier.
        let order = Port {
            contract_id: Some("order".into()),
            ..Port::new("order", ValueType::Object)
        };
        assert_eq!(Pill::of(&order, &[]).text(), "order * object · order");
    }
}

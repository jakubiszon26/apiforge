//! Small widgets shared across panels, so spacing and typography stay consistent.

use crate::{dnd::Payload, theme};
use eframe::egui::{
    self, Align2, FontId, Response, RichText, Sense, Stroke, StrokeKind, Ui, Widget, vec2,
};

/// An uppercase panel caption.
pub fn caption(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(10.0).strong().color(theme::MUTED));
}

/// A smaller caption inside a section.
pub fn subcaption(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(9.5).color(theme::MUTED));
}

/// A muted explanatory line.
pub fn hint(ui: &mut Ui, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).size(11.0).color(theme::MUTED));
}

/// The label above a form field.
pub fn field_label(ui: &mut Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(text).size(11.5).color(theme::MUTED));
}

/// A titled block of a form.
pub fn section(ui: &mut Ui, title: &str) {
    ui.add_space(10.0);
    ui.label(RichText::new(title).size(12.5).family(theme::bold_family()));
    ui.add_space(2.0);
}

pub fn ellipsis(text: &str, limit: usize) -> String {
    crate::labels::truncate(text, limit)
}

/// A full-width, selectable navigation row.
pub struct NavItem<'a> {
    text: &'a str,
    active: bool,
}

impl<'a> NavItem<'a> {
    pub fn new(text: &'a str, active: bool) -> Self {
        Self { text, active }
    }
}

impl Widget for NavItem<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());
        let painter = ui.painter();
        if self.active {
            painter.rect_filled(rect, 7.0, theme::ACCENT_SOFT);
        } else if response.hovered() {
            painter.rect_filled(rect, 7.0, theme::BG);
        }
        let color = if !ui.is_enabled() {
            theme::MUTED
        } else if self.active {
            theme::ACCENT
        } else {
            theme::TEXT
        };
        painter.with_clip_rect(rect.shrink(4.0)).text(
            rect.left_center() + vec2(10.0, 0.0),
            Align2::LEFT_CENTER,
            self.text,
            FontId::proportional(12.5),
            color,
        );
        if self.active {
            painter.circle_filled(rect.right_center() - vec2(12.0, 0.0), 3.0, theme::ACCENT);
        }
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    }
}

/// A collapsible group header with a count.
pub struct Disclosure<'a> {
    text: &'a str,
    open: bool,
    count: usize,
}

impl<'a> Disclosure<'a> {
    pub fn new(text: &'a str, open: bool, count: usize) -> Self {
        Self { text, open, count }
    }
}

impl Widget for Disclosure<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
        let painter = ui.painter();
        if response.hovered() {
            painter.rect_filled(rect, 5.0, theme::BG);
        }
        painter.text(
            rect.left_center() + vec2(6.0, 0.0),
            Align2::LEFT_CENTER,
            if self.open { "▾" } else { "▸" },
            FontId::proportional(11.0),
            theme::MUTED,
        );
        painter.text(
            rect.left_center() + vec2(20.0, 0.0),
            Align2::LEFT_CENTER,
            self.text,
            FontId::proportional(12.0),
            theme::TEXT,
        );
        painter.text(
            rect.right_center() - vec2(6.0, 0.0),
            Align2::RIGHT_CENTER,
            self.count.to_string(),
            FontId::proportional(10.0),
            theme::MUTED,
        );
        response
    }
}

/// A draggable variable source; `kind` is a short label such as "parameter".
pub fn variable_chip(ui: &mut Ui, label: &str, kind: &str, payload: Payload) -> Response {
    let text = egui::WidgetText::from(RichText::new(label).size(11.0).color(theme::VARIABLE));
    let galley = text.into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        180.0,
        egui::TextStyle::Button,
    );
    let size = galley.size() + vec2(18.0, 10.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    response.dnd_set_drag_payload(payload);
    let fill = if response.hovered() {
        theme::VARIABLE.gamma_multiply(0.2)
    } else {
        theme::VARIABLE.gamma_multiply(0.1)
    };
    ui.painter().rect(
        rect,
        11.0,
        fill,
        Stroke::new(1.0, theme::VARIABLE.gamma_multiply(0.35)),
        StrokeKind::Inside,
    );
    ui.painter()
        .galley(rect.min + vec2(9.0, 5.0), galley, theme::VARIABLE);
    response
        .on_hover_text(format!(
            "{label} · {kind} — drag onto an argument or into text"
        ))
        .on_hover_cursor(egui::CursorIcon::Grab)
}

/// A rounded status label.
pub fn pill(ui: &mut Ui, text: &str, color: egui::Color32) -> Response {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.12))
        .corner_radius(10)
        .inner_margin(egui::Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(10.0).strong().color(color))
        })
        .response
}

/// The main call-to-action button.
pub fn primary_button(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_owned()).color(theme::WHITE).strong())
        .fill(theme::ACCENT)
        .stroke(Stroke::NONE)
        .min_size(vec2(0.0, 34.0))
}

/// A validation message shown directly below the field it describes.
pub fn field_error(ui: &mut Ui, text: impl Into<String>) {
    ui.add(egui::Label::new(RichText::new(text.into()).size(11.0).color(theme::DANGER)).wrap());
}

/// Draws `add_contents` with a red outline on its fields when `invalid`.
pub fn validated<R>(ui: &mut Ui, invalid: bool, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    ui.scope(|ui| {
        if invalid {
            let stroke = Stroke::new(1.0, theme::DANGER);
            let widgets = &mut ui.visuals_mut().widgets;
            widgets.inactive.bg_stroke = stroke;
            widgets.hovered.bg_stroke = stroke;
            ui.visuals_mut().selection.stroke = stroke;
        }
        add_contents(ui)
    })
    .inner
}

/// A tinted block listing the problems that prevent saving.
pub fn error_box(ui: &mut Ui, messages: &[String]) {
    if messages.is_empty() {
        return;
    }
    egui::Frame::new()
        .fill(theme::DANGER.gamma_multiply(0.08))
        .stroke(Stroke::new(1.0, theme::DANGER.gamma_multiply(0.3)))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            for message in messages {
                ui.add(
                    egui::Label::new(RichText::new(message).size(11.5).color(theme::DANGER)).wrap(),
                );
            }
        });
}

/// A small frameless button for row actions such as removing an entry.
pub fn icon_button(text: &str) -> egui::Button<'static> {
    egui::Button::new(
        RichText::new(text.to_owned())
            .size(12.0)
            .color(theme::MUTED),
    )
    .frame(false)
    .min_size(vec2(24.0, 24.0))
}

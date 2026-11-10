//! Small widgets shared across panels, so spacing and typography stay consistent.

pub use crate::icons::Icon;
use crate::{dnd::Payload, theme};
use eframe::egui::{
    self, Align2, FontId, Response, RichText, Sense, Stroke, StrokeKind, Ui, Widget, vec2,
};

/// Unscaled form controls share a baseline, inset and target height.
pub fn text_edit(text: &mut String) -> egui::TextEdit<'_> {
    egui::TextEdit::singleline(text)
        .min_size(vec2(0.0, theme::CONTROL_HEIGHT))
        .margin(egui::Margin::symmetric(10, 6))
        .vertical_align(egui::Align::Center)
}

pub fn text_area(text: &mut String) -> egui::TextEdit<'_> {
    egui::TextEdit::multiline(text).margin(egui::Margin::symmetric(10, 8))
}

/// A single line laid out to its own cell, rather than clipping oversized text
/// against the whole card. Every label has a width budget and an ellipsis.
pub fn paint_label(
    painter: &egui::Painter,
    rect: egui::Rect,
    text: &str,
    font: FontId,
    color: egui::Color32,
    align: egui::Align,
) {
    if rect.width() < 2.0 || rect.height() < 2.0 {
        return;
    }
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap.max_width = rect.width();
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = painter.layout_job(job);
    let x = match align {
        egui::Align::Min => rect.left(),
        egui::Align::Center => rect.center().x - galley.size().x / 2.0,
        egui::Align::Max => rect.right() - galley.size().x,
    };
    let at = egui::pos2(x, rect.center().y - galley.size().y / 2.0);
    painter
        .with_clip_rect(rect.intersect(painter.clip_rect()))
        .galley(at, galley, color);
}

/// An uppercase panel caption.
pub fn caption(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .size(theme::FONT_META)
            .family(theme::bold_family())
            .color(theme::MUTED),
    );
}

/// A smaller caption inside a section.
pub fn subcaption(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .size(theme::FONT_META)
            .color(theme::MUTED),
    );
}

/// A muted explanatory line.
pub fn hint(ui: &mut Ui, text: impl Into<String>) {
    ui.label(
        RichText::new(text.into())
            .size(theme::FONT_LABEL)
            .color(theme::MUTED),
    );
}

/// Gives a control the accessible name a screen reader announces. A field's
/// visible label is a separate widget, and a hint or tooltip is not a name.
pub fn named(response: egui::Response, name: impl Into<String>) -> egui::Response {
    let name = name.into();
    response
        .ctx
        .accesskit_node_builder(response.id, |node| node.set_label(name));
    response
}

/// Names the resize handle of a panel shown this frame. egui gives the
/// handle, which keyboard focus can reach, an ID derived from the panel's and
/// no name; one it did not draw is left alone rather than invented.
pub fn name_resize_handle(ctx: &egui::Context, panel: egui::Id, name: &str) {
    let handle = panel.with("__resize");
    if ctx.read_response(handle).is_some() {
        ctx.accesskit_node_builder(handle, |node| node.set_label(name));
    }
}

/// The place a dragged panel takes when it is released there (UX-05): a soft
/// accent fill with an accent outline and `label` in bold accent text, painted
/// above every panel. Its rect is a labelled widget that takes no click, so
/// the zone is found by its words and the drag under it goes on.
pub fn drop_zone(ui: &Ui, rect: egui::Rect, label: &str) {
    let id = egui::Id::new(("drop-zone", label));
    let painter = ui
        .ctx()
        .layer_painter(egui::LayerId::new(egui::Order::Foreground, id));
    painter.rect(
        rect,
        6,
        theme::ACCENT_SOFT,
        Stroke::new(1.5, theme::ACCENT),
        StrokeKind::Inside,
    );
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::new(theme::FONT_BODY, theme::bold_family()),
        theme::ACCENT,
    );
    let label = label.to_owned();
    ui.interact(rect, id, Sense::hover())
        .widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &label));
}

pub fn field_label(ui: &mut Ui, text: &str) {
    let _ = field_name(ui, text);
}

/// The label above a form field; the field is named by it with
/// `labelled_by(response.id)`.
pub fn field_name(ui: &mut Ui, text: &str) -> egui::Response {
    ui.add_space(theme::SPACE_SM);
    ui.label(
        RichText::new(text)
            .size(theme::FONT_LABEL)
            .color(theme::MUTED),
    )
}

/// The accessible name of a widget without a visible label.
pub fn name(ui: &Ui, id: egui::Id, name: &str) {
    ui.ctx()
        .accesskit_node_builder(id, |node| node.set_label(name));
}

/// A titled block of a form.
pub fn section(ui: &mut Ui, title: &str) {
    ui.add_space(theme::SPACE_LG);
    ui.label(
        RichText::new(title)
            .size(theme::FONT_BODY)
            .family(theme::bold_family()),
    );
    ui.add_space(4.0);
}

/// A section title rendered inside a horizontal row (without adding horizontal pre-spacing).
pub fn section_inline(ui: &mut Ui, title: &str) {
    ui.label(
        RichText::new(title)
            .size(theme::FONT_BODY)
            .family(theme::bold_family()),
    );
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
        paint_label(
            painter,
            egui::Rect::from_min_max(
                rect.left_top() + vec2(10.0, 4.0),
                rect.right_bottom() - vec2(24.0, 4.0),
            ),
            self.text,
            FontId::proportional(theme::FONT_BODY),
            color,
            egui::Align::Min,
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
        let open = self.open;
        response
            .ctx
            .accesskit_node_builder(response.id, |node| node.set_expanded(open));
        named(response, self.text)
    }
}

/// A draggable variable source; `kind` is a short label such as "parameter".
pub fn variable_chip(ui: &mut Ui, label: &str, kind: &str, payload: Payload) -> Response {
    let text = egui::WidgetText::from(
        RichText::new(label)
            .size(theme::FONT_LABEL)
            .color(theme::VARIABLE),
    );
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
    named(
        response
            .on_hover_text(format!(
                "{label} · {kind} — drag onto an argument, into text, or onto the board to place it there"
            ))
            .on_hover_cursor(egui::CursorIcon::Grab),
        format!("{label} · {kind}"),
    )
}

/// A rounded status label.
pub fn pill(ui: &mut Ui, text: &str, color: egui::Color32) -> Response {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.12))
        .corner_radius(10)
        .inner_margin(egui::Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.label(
                RichText::new(text)
                    .size(theme::FONT_META)
                    .strong()
                    .color(color),
            )
        })
        .response
}

/// The main call-to-action button.
pub fn primary_button(text: &str) -> ActionButton<'_> {
    action(Icon::Check, text).primary()
}

/// A validation message shown directly below the field it describes.
pub fn field_error(ui: &mut Ui, text: impl Into<String>) {
    ui.add(
        egui::Label::new(
            RichText::new(text.into())
                .size(theme::FONT_LABEL)
                .color(theme::DANGER),
        )
        .wrap(),
    );
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
                    egui::Label::new(
                        RichText::new(message)
                            .size(theme::FONT_LABEL)
                            .color(theme::DANGER),
                    )
                    .wrap(),
                );
            }
        });
}

#[derive(Clone, Copy, PartialEq)]
enum ButtonStyle {
    Secondary,
    Quiet,
    Primary,
    Danger,
}

/// Every action uses the same icon grid, baseline, focus ring and hit target.
pub struct ActionButton<'a> {
    icon: Icon,
    label: &'a str,
    icon_only: bool,
    style: ButtonStyle,
    compact: bool,
    /// A toggle, pressed or released: a pressed one has a soft accent fill,
    /// an accent border and an accent icon, and is announced with its state.
    pressed: Option<bool>,
    /// The accessible name, when it says more than the visible label.
    name: Option<&'a str>,
    /// A primary button's fill instead of the accent.
    fill: Option<egui::Color32>,
}

pub fn action(icon: Icon, label: &str) -> ActionButton<'_> {
    ActionButton {
        icon,
        label,
        icon_only: false,
        style: ButtonStyle::Secondary,
        compact: false,
        pressed: None,
        name: None,
        fill: None,
    }
}

pub fn icon_button(icon: Icon, label: &str) -> ActionButton<'_> {
    ActionButton {
        icon_only: true,
        style: ButtonStyle::Quiet,
        ..action(icon, label)
    }
}

impl ActionButton<'_> {
    pub fn primary(mut self) -> Self {
        self.style = ButtonStyle::Primary;
        self
    }
    pub fn quiet(mut self) -> Self {
        self.style = ButtonStyle::Quiet;
        self
    }
    pub fn danger(mut self) -> Self {
        self.style = ButtonStyle::Danger;
        self
    }
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }
    /// A toggle button, pressed or released.
    pub fn toggle(mut self, pressed: bool) -> Self {
        self.pressed = Some(pressed);
        self
    }
    /// A primary button in `colour` instead of the accent, as *Run
    /// simulation* is (UX-02).
    pub fn fill(mut self, colour: egui::Color32) -> Self {
        self.fill = Some(colour);
        self
    }

    /// A primary button's fill at rest.
    pub fn primary_fill(&self) -> egui::Color32 {
        self.fill.unwrap_or(theme::ACCENT)
    }
}

impl<'a> ActionButton<'a> {
    /// Names the button for assistive technology instead of its label.
    pub fn named(mut self, name: &'a str) -> Self {
        self.name = Some(name);
        self
    }
}

impl Widget for ActionButton<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let height = if self.compact {
            26.0
        } else {
            theme::CONTROL_HEIGHT
        };
        let icon_size = if self.compact { 14.0 } else { 16.0 };
        let font = FontId::new(
            if self.compact {
                theme::FONT_LABEL
            } else {
                theme::FONT_BODY
            },
            if self.style == ButtonStyle::Primary {
                theme::bold_family()
            } else {
                egui::FontFamily::Proportional
            },
        );
        let galley = ui
            .painter()
            .layout_no_wrap(self.label.to_owned(), font.clone(), theme::TEXT);
        let natural_width = if self.icon_only {
            height
        } else {
            20.0 + icon_size + 6.0 + galley.size().x
        };
        let width = natural_width.min(ui.available_width()).max(height);
        let (rect, response) = ui.allocate_at_least(vec2(width, height), Sense::click());
        let name = self.name.unwrap_or(self.label);
        response.widget_info(|| match self.pressed {
            Some(pressed) => {
                egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), pressed, name)
            }
            None => egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), name),
        });
        let pressed = self.pressed == Some(true) && ui.is_enabled();
        if ui.is_rect_visible(rect) {
            // A disabled button must look disabled: it is painted here, so egui
            // does not fade it, and a full-strength accent reads as live.
            let enabled = ui.is_enabled();
            let engaged = enabled && (response.hovered() || response.is_pointer_button_down_on());
            let primary = self.style == ButtonStyle::Primary;
            let fill = if !enabled {
                if primary {
                    theme::FIELD
                } else {
                    egui::Color32::TRANSPARENT
                }
            } else if primary {
                // Another fill darkens as the accent does: by 8 % under the
                // pointer and by 17 % while pressed.
                match (
                    self.fill,
                    response.is_pointer_button_down_on(),
                    response.hovered(),
                ) {
                    (None, true, _) => theme::ACCENT_PRESSED,
                    (None, false, true) => theme::ACCENT_HOVER,
                    (Some(colour), true, _) => darker(colour, 0.83),
                    (Some(colour), false, true) => darker(colour, 0.92),
                    (_, false, false) => self.primary_fill(),
                }
            } else if pressed {
                theme::ACCENT_SOFT
            } else if engaged {
                theme::FIELD
            } else {
                egui::Color32::TRANSPARENT
            };
            let border = if enabled && response.has_focus() {
                Stroke::new(1.5, theme::ACCENT)
            } else if pressed {
                Stroke::new(1.0, theme::ACCENT)
            } else if self.style == ButtonStyle::Secondary || (!enabled && primary) {
                Stroke::new(1.0, theme::BORDER)
            } else {
                Stroke::NONE
            };
            ui.painter()
                .rect(rect.shrink(0.5), 5, fill, border, egui::StrokeKind::Inside);
            let color = if !enabled {
                theme::MUTED
            } else if primary {
                theme::WHITE
            } else if self.style == ButtonStyle::Danger {
                theme::DANGER
            } else {
                theme::TEXT
            };
            let icon_color = if pressed {
                theme::ACCENT
            } else if !primary && self.style != ButtonStyle::Danger {
                theme::MUTED
            } else {
                color
            };
            let icon = egui::Rect::from_center_size(
                if self.icon_only {
                    rect.center()
                } else {
                    egui::pos2(rect.left() + 10.0 + icon_size / 2.0, rect.center().y)
                },
                vec2(icon_size, icon_size),
            );
            self.icon.paint(ui.painter(), icon, icon_color);
            if !self.icon_only {
                paint_label(
                    ui.painter(),
                    egui::Rect::from_min_max(
                        egui::pos2(icon.right() + 6.0, rect.top()),
                        egui::pos2(rect.right() - 10.0, rect.bottom()),
                    ),
                    self.label,
                    font,
                    color,
                    egui::Align::Min,
                );
            }
        }
        let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        if self.icon_only {
            response.on_hover_text(self.label)
        } else {
            response
        }
    }
}

pub fn action_menu(
    ui: &mut Ui,
    button: ActionButton<'_>,
    content: impl FnOnce(&mut Ui),
) -> Response {
    let name = menu_name(&button);
    let response = ui.add(button);
    show_menu(&response, &name, content);
    response
}

/// [`action_menu`] with its button at `rect`, a place `ui` took earlier: the
/// button then follows the widgets drawn meanwhile in the Tab order. The
/// cursor of `ui` stays where it is.
pub fn action_menu_at(
    ui: &mut Ui,
    rect: egui::Rect,
    button: ActionButton<'_>,
    content: impl FnOnce(&mut Ui),
) -> Response {
    let name = menu_name(&button);
    let response = ui
        .new_child(egui::UiBuilder::new().max_rect(rect).layout(
            egui::Layout::centered_and_justified(egui::Direction::TopDown),
        ))
        .add(button);
    show_menu(&response, &name, content);
    response
}

fn menu_name(button: &ActionButton<'_>) -> String {
    format!("{} menu", button.label.trim_end_matches('…'))
}

fn show_menu(response: &Response, name: &str, content: impl FnOnce(&mut Ui)) {
    egui::Popup::menu(response).show(|ui| {
        name_menu(ui, name);
        content(ui)
    });
}

/// `colour` with its red, green and blue scaled by `factor`.
fn darker(colour: egui::Color32, factor: f32) -> egui::Color32 {
    let scale = |channel: u8| (f32::from(channel) * factor).round() as u8;
    egui::Color32::from_rgb(scale(colour.r()), scale(colour.g()), scale(colour.b()))
}

/// Names the open menu or popup whose contents `ui` draws. egui lets its
/// area take keyboard focus, so it is a focus stop that needs a name.
pub fn name_menu(ui: &Ui, name: &str) {
    ui.ctx()
        .accesskit_node_builder(ui.layer_id().id.with("move"), |node| {
            node.set_role(egui::accesskit::Role::Menu);
            node.set_label(name);
        });
}

/// Tabs use a thin active indicator instead of looking like action buttons.
pub fn tab_value<T: PartialEq>(
    ui: &mut Ui,
    value: &mut T,
    candidate: T,
    label: impl AsRef<str>,
) -> Response {
    let text = label.as_ref();
    let selected = *value == candidate;
    let font = FontId::proportional(theme::FONT_BODY);
    let width = tab_value_width(ui, text);
    let (rect, mut response) =
        ui.allocate_at_least(vec2(width, theme::CONTROL_HEIGHT), Sense::click());
    if response.clicked() && !selected {
        *value = candidate;
        response.mark_changed();
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            text,
        )
    });
    if response.hovered() {
        ui.painter().rect_filled(rect, 3, theme::FIELD);
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(1.0),
            3,
            Stroke::new(1.0, theme::ACCENT),
            egui::StrokeKind::Inside,
        );
    }
    if selected {
        ui.painter().line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(2.0, theme::ACCENT),
        );
    }
    paint_label(
        ui.painter(),
        rect.shrink2(vec2(12.0, 0.0)),
        text,
        font,
        if selected { theme::TEXT } else { theme::MUTED },
        egui::Align::Center,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The width [`tab_value`] takes for `label`.
pub fn tab_value_width(ui: &Ui, label: &str) -> f32 {
    ui.painter()
        .layout_no_wrap(
            label.to_owned(),
            FontId::proportional(theme::FONT_BODY),
            theme::TEXT,
        )
        .size()
        .x
        + 24.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn click(at: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn key(key: egui::Key, pressed: bool) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn frame(
        context: &egui::Context,
        events: Vec<egui::Event>,
        enabled: bool,
        menu: bool,
    ) -> (Response, bool) {
        let mut response = None;
        let mut menu_visible = false;
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    vec2(640.0, 480.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                response = Some(if menu {
                    action_menu(ui, icon_button(Icon::More, "Project actions"), |ui| {
                        menu_visible = true;
                        ui.label("Import project");
                    })
                } else {
                    ui.add_enabled(enabled, action(Icon::Play, "Run test").primary())
                });
            },
        );
        output.textures_delta.clear();
        (response.unwrap(), menu_visible)
    }

    #[test]
    fn symbolic_actions_keep_mouse_keyboard_and_disabled_behavior() {
        let context = egui::Context::default();
        theme::apply(&context);
        let (button, _) = frame(&context, vec![], true, false);
        let at = button.rect.center();
        frame(
            &context,
            vec![egui::Event::PointerMoved(at), click(at, true)],
            true,
            false,
        );
        assert!(
            frame(&context, vec![click(at, false)], true, false)
                .0
                .clicked()
        );
        button.request_focus();
        assert!(
            frame(&context, vec![key(egui::Key::Space, true)], true, false)
                .0
                .clicked()
        );
        frame(&context, vec![key(egui::Key::Space, false)], true, false);
        frame(&context, vec![click(at, true)], false, false);
        assert!(
            !frame(&context, vec![click(at, false)], false, false)
                .0
                .clicked()
        );
        assert!(
            !frame(&context, vec![key(egui::Key::Enter, true)], false, false)
                .0
                .clicked()
        );
    }

    #[test]
    fn symbolic_menu_opens_and_escape_dismisses_it() {
        let context = egui::Context::default();
        theme::apply(&context);
        let (button, visible) = frame(&context, vec![], true, true);
        assert!(!visible);
        let at = button.rect.center();
        frame(
            &context,
            vec![egui::Event::PointerMoved(at), click(at, true)],
            true,
            true,
        );
        assert!(frame(&context, vec![click(at, false)], true, true).1);
        frame(&context, vec![key(egui::Key::Escape, true)], true, true);
        assert!(!frame(&context, vec![], true, true).1);
    }
}

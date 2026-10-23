//! Visual tokens shared by panels and the board, and the egui style built from them.
use apiforge_core::model::NodeKind;
use eframe::egui::{self, Color32, FontId, Stroke, TextStyle};

pub const BG: Color32 = Color32::from_rgb(246, 248, 251);
pub const WHITE: Color32 = Color32::WHITE;
pub const PANEL: Color32 = Color32::from_rgb(251, 252, 254);
pub const TEXT: Color32 = Color32::from_rgb(32, 39, 51);
pub const MUTED: Color32 = Color32::from_rgb(108, 117, 132);
pub const BORDER: Color32 = Color32::from_rgb(225, 230, 237);
pub const ACCENT: Color32 = Color32::from_rgb(216, 91, 44);
pub const LINK: Color32 = Color32::from_rgb(70, 106, 145);
pub const FIELD: Color32 = Color32::from_rgb(246, 248, 251);
pub const GRID: Color32 = Color32::from_rgb(207, 215, 226);
pub const SOCKET: Color32 = Color32::from_rgb(166, 176, 190);
pub const MINIMAP: Color32 = Color32::from_rgb(222, 228, 237);
pub const SUCCESS: Color32 = Color32::from_rgb(61, 140, 92);
pub const DANGER: Color32 = Color32::from_rgb(196, 64, 48);
pub const WARNING: Color32 = Color32::from_rgb(196, 134, 32);
pub const DATA_WIRE: Color32 = Color32::from_rgb(167, 178, 193);
pub const CONTROL_WIRE: Color32 = Color32::from_rgb(143, 157, 177);
pub const BRANCH_YES: Color32 = Color32::from_rgb(77, 138, 97);
pub const BRANCH_NO: Color32 = Color32::from_rgb(181, 112, 76);
pub const OPERATOR: Color32 = Color32::from_rgb(58, 138, 96);
pub const VARIABLE: Color32 = Color32::from_rgb(188, 105, 59);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(251, 233, 221);

pub const SPACE_SM: f32 = 8.0;
pub const SPACE_MD: f32 = 12.0;
pub const SPACE_LG: f32 = 16.0;
pub const SPACE_XL: f32 = 24.0;
pub const FONT_META: f32 = 11.0;
pub const FONT_LABEL: f32 = 12.0;
pub const FONT_BODY: f32 = 13.0;
pub const FONT_TITLE: f32 = 20.0;
pub const FONT_PANEL_TITLE: f32 = 20.0;
pub const CONTROL_HEIGHT: f32 = 32.0;
pub const SIDEBAR_WIDTH: f32 = 256.0;
pub const COMPACT_SIDEBAR_WIDTH: f32 = 220.0;

pub fn bold_family() -> egui::FontFamily {
    egui::FontFamily::Name("api-forge-bold".into())
}

pub fn apply(ctx: &egui::Context) {
    // Ship the same fonts on Linux and Windows; system fallback must not turn
    // arrows and UI symbols into missing-glyph boxes or change card geometry.
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "api-forge-regular".into(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/APIFORGE-Sans-Regular.ttf"))
            .into(),
    );
    fonts.font_data.insert(
        "api-forge-bold".into(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/APIFORGE-Sans-Semibold.ttf"))
            .into(),
    );
    fonts.font_data.insert(
        "api-forge-mono".into(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/AdwaitaMono-Regular.ttf"))
            .into(),
    );
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "api-forge-mono".into());
    fonts.font_data.insert(
        "api-forge-symbols".into(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/DejaVuSans.ttf")).into(),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "api-forge-regular".into());
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .push("api-forge-symbols".into());
    fonts.families.insert(
        bold_family(),
        vec!["api-forge-bold".into(), "api-forge-symbols".into()],
    );
    ctx.set_fonts(fonts);
    ctx.set_theme(egui::Theme::Light);
    let mut style = (*ctx.style_of(egui::Theme::Light)).clone();
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(FONT_BODY));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(FONT_BODY));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(FONT_META));
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::proportional(FONT_PANEL_TITLE));
    style
        .text_styles
        .insert(TextStyle::Monospace, FontId::monospace(12.0));
    style.spacing.item_spacing = egui::vec2(SPACE_SM, 6.0);
    style.spacing.button_padding = egui::vec2(SPACE_SM, 6.0);
    style.spacing.interact_size = egui::vec2(CONTROL_HEIGHT, CONTROL_HEIGHT);
    style.spacing.window_margin = egui::Margin::same(SPACE_LG as i8);
    style.spacing.menu_margin = egui::Margin::same(SPACE_MD as i8);
    let visuals = &mut style.visuals;
    *visuals = egui::Visuals::light();
    visuals.override_text_color = Some(TEXT);
    visuals.weak_text_color = Some(MUTED);
    visuals.panel_fill = WHITE;
    visuals.window_fill = WHITE;
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    visuals.window_corner_radius = egui::CornerRadius::same(10);
    visuals.extreme_bg_color = BG;
    visuals.faint_bg_color = BG;
    visuals.text_edit_bg_color = Some(WHITE);
    visuals.code_bg_color = BG;
    visuals.hyperlink_color = LINK;
    visuals.selection.bg_fill = Color32::from_rgb(251, 233, 221);
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::same(6);
        widget.bg_stroke = Stroke::new(1.0, BORDER);
        widget.fg_stroke = Stroke::new(1.0, TEXT);
        widget.expansion = 0.0;
    }
    visuals.widgets.inactive.bg_fill = WHITE;
    visuals.widgets.inactive.weak_bg_fill = WHITE;
    visuals.widgets.hovered.bg_fill = BG;
    visuals.widgets.hovered.weak_bg_fill = BG;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(184, 194, 208));
    visuals.widgets.active.bg_fill = Color32::from_rgb(251, 233, 221);
    visuals.widgets.active.weak_bg_fill = visuals.widgets.active.bg_fill;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    ctx.set_style_of(egui::Theme::Light, style);
}

pub fn kind_color(kind: NodeKind) -> Color32 {
    match kind {
        NodeKind::Request | NodeKind::Select => Color32::from_rgb(100, 118, 184),
        NodeKind::Assert | NodeKind::Condition | NodeKind::Trigger | NodeKind::If => {
            Color32::from_rgb(77, 138, 97)
        }
        NodeKind::Formula | NodeKind::Build => Color32::from_rgb(152, 110, 177),
        NodeKind::Component | NodeKind::Variable => Color32::from_rgb(188, 105, 59),
    }
}

/// Code-native icons avoid platform-specific symbol-font substitutions.
pub fn kind_icon(painter: &egui::Painter, rect: egui::Rect, kind: NodeKind, color: Color32) {
    let center = rect.center();
    let radius = rect.width() * 0.25;
    let stroke = Stroke::new((rect.width() / 24.0).max(0.9), color);
    match kind {
        NodeKind::Request => {
            painter.circle_stroke(center, radius, stroke);
            painter.line_segment(
                [
                    center - egui::vec2(radius, 0.0),
                    center + egui::vec2(radius, 0.0),
                ],
                stroke,
            );
            painter.add(egui::Shape::closed_line(
                (0..24)
                    .map(|index| {
                        let angle = std::f32::consts::TAU * index as f32 / 24.0;
                        center + egui::vec2(angle.cos() * radius * 0.45, angle.sin() * radius)
                    })
                    .collect(),
                stroke,
            ));
        }
        NodeKind::Component => {
            let points: Vec<_> = (0..6)
                .map(|index| {
                    let angle = std::f32::consts::TAU * index as f32 / 6.0;
                    center + egui::vec2(angle.sin(), angle.cos()) * radius
                })
                .collect();
            painter.add(egui::Shape::closed_line(points.clone(), stroke));
            for index in [0, 2, 4] {
                painter.line_segment([center, points[index]], stroke);
            }
        }
        NodeKind::Assert => {
            painter.line_segment(
                [
                    center + egui::vec2(-radius, 0.0),
                    center + egui::vec2(-radius * 0.2, radius * 0.65),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + egui::vec2(-radius * 0.2, radius * 0.65),
                    center + egui::vec2(radius, -radius * 0.7),
                ],
                stroke,
            );
        }
        NodeKind::If | NodeKind::Condition => {
            for sign in [-1.0, 1.0] {
                painter.line_segment([center + egui::vec2(0.0, radius), center], stroke);
                painter.line_segment(
                    [center, center + egui::vec2(radius * sign, -radius)],
                    stroke,
                );
                painter.circle_stroke(
                    center + egui::vec2(radius * sign, -radius),
                    radius * 0.25,
                    stroke,
                );
            }
        }
        NodeKind::Trigger => {
            painter.circle_stroke(center, radius, stroke);
            painter.add(egui::Shape::convex_polygon(
                vec![
                    center + egui::vec2(-radius * 0.25, -radius * 0.45),
                    center + egui::vec2(-radius * 0.25, radius * 0.45),
                    center + egui::vec2(radius * 0.5, 0.0),
                ],
                color,
                Stroke::NONE,
            ));
        }
        NodeKind::Select => {
            painter.add(egui::Shape::line(
                vec![
                    center + egui::vec2(-radius, -radius),
                    center + egui::vec2(radius, -radius),
                    center + egui::vec2(0.2 * radius, 0.0),
                    center + egui::vec2(0.2 * radius, radius),
                    center + egui::vec2(-0.2 * radius, radius),
                    center + egui::vec2(-0.2 * radius, 0.0),
                    center + egui::vec2(-radius, -radius),
                ],
                stroke,
            ));
        }
        _ => {
            painter.text(
                center,
                egui::Align2::CENTER_CENTER,
                if kind == NodeKind::Formula {
                    "ƒ"
                } else {
                    "{}"
                },
                egui::FontId::proportional(rect.width() * 0.55),
                color,
            );
        }
    }
}

//! Drawing of the board. Nothing here handles input.

use super::{
    Camera,
    layout::{Card, RowValue, WIDTH},
};
use crate::{icons::Icon, theme, widgets::paint_label as label};
use apiforge_core::{
    format::Graph,
    model::{Branch, NodeKind},
    run::{StepResult, StepStatus},
};
use eframe::egui::{
    self, Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2,
    epaint::CubicBezierShape, vec2,
};
use std::collections::HashSet;

pub fn grid(painter: &Painter, viewport: Rect, camera: Camera) {
    let mut spacing = 32.0 * camera.zoom;
    while spacing < 16.0 {
        spacing *= 2.0;
    }
    let origin = camera.to_screen(Pos2::ZERO);
    let start = viewport.min
        + vec2(
            (origin.x - viewport.min.x).rem_euclid(spacing),
            (origin.y - viewport.min.y).rem_euclid(spacing),
        );
    let mut x = start.x;
    while x < viewport.right() {
        let mut y = start.y;
        while y < viewport.bottom() {
            painter.circle_filled(
                egui::pos2(x, y),
                0.65,
                theme::GRID.gamma_multiply((camera.zoom * 0.8).clamp(0.3, 0.8)),
            );
            y += spacing;
        }
        x += spacing;
    }
}

/// Control points of a wire: control wires leave downwards, data wires sideways.
pub fn wire_points(start: Pos2, end: Pos2, control: bool, zoom: f32) -> [Pos2; 4] {
    let bend = if control {
        vec2(
            0.0,
            (((end.y - start.y).abs() * 0.5).max(40.0 * zoom)).min(160.0 * zoom),
        )
    } else {
        vec2(((end.x - start.x).abs() * 0.45).max(40.0 * zoom), 0.0)
    };
    [start, start + bend, end - bend, end]
}

pub fn bezier(points: [Pos2; 4], t: f32) -> Pos2 {
    let u = 1.0 - t;
    let [a, b, c, d] = points.map(Pos2::to_vec2);
    (a * u * u * u + b * 3.0 * u * u * t + c * 3.0 * u * t * t + d * t * t * t).to_pos2()
}

pub fn wire(
    painter: &Painter,
    start: Pos2,
    end: Pos2,
    control: bool,
    branch: Option<Branch>,
    selected: bool,
    zoom: f32,
) {
    let color = match (selected, control, branch) {
        (true, ..) => theme::ACCENT,
        (_, true, Some(Branch::False)) => theme::BRANCH_NO,
        (_, true, Some(Branch::True)) => theme::BRANCH_YES,
        (_, true, None) => theme::CONTROL_WIRE,
        (_, false, _) => theme::DATA_WIRE,
    };
    let width = if selected { 2.4 } else { 1.6 };
    let points = wire_points(start, end, control, zoom);
    if control {
        // Dashed: sample the curve and draw every other segment.
        let samples: Vec<Pos2> = (0..=32)
            .map(|step| bezier(points, step as f32 / 32.0))
            .collect();
        for pair in samples.windows(2).step_by(2) {
            painter.line_segment([pair[0], pair[1]], Stroke::new(width, color));
        }
    } else {
        painter.add(CubicBezierShape::from_points_stroke(
            points,
            false,
            Color32::TRANSPARENT,
            Stroke::new(width, color),
        ));
    }
    let direction = (end - bezier(points, 0.92)).normalized();
    let normal = vec2(-direction.y, direction.x);
    let size = 6.0 * zoom.max(0.6);
    painter.add(Shape::convex_polygon(
        vec![
            end,
            end - direction * size + normal * size * 0.5,
            end - direction * size - normal * size * 0.5,
        ],
        color,
        Stroke::NONE,
    ));
    if control
        && zoom >= 0.6
        && let Some(branch) = branch
    {
        painter.text(
            bezier(points, 0.5) + vec2(6.0, 0.0),
            Align2::LEFT_CENTER,
            match branch {
                Branch::True => "yes",
                Branch::False => "no",
            },
            FontId::proportional(10.0 * zoom),
            color,
        );
    }
}

fn status_color(status: StepStatus) -> Color32 {
    match status {
        StepStatus::Passed => theme::SUCCESS,
        StepStatus::Failed | StepStatus::Error => theme::DANGER,
        StepStatus::Blocked | StepStatus::Cancelled => theme::WARNING,
        StepStatus::Skipped => theme::MUTED,
    }
}

pub fn card(
    painter: &Painter,
    card: &Card,
    camera: Camera,
    selected: bool,
    step: Option<&StepResult>,
) {
    use super::layout::{CARD_PADDING, CardDetail};
    let z = camera.zoom;
    let detail = CardDetail::at_zoom(z);
    let rect = camera.rect(card.rect());
    let local = |x: f32, y: f32| camera.to_screen(card.origin + vec2(x, y));
    let cell = |left: f32, top: f32, right: f32, bottom: f32| {
        Rect::from_min_max(local(left, top), local(right, bottom))
    };
    let radius = (10.0 * z).max(3.0);
    painter.rect_filled(
        rect.translate(vec2(0.0, 2.0 * z)),
        radius,
        Color32::from_black_alpha(8),
    );
    painter.rect_filled(rect, radius, theme::WHITE);
    let border_color = if selected {
        theme::ACCENT
    } else {
        theme::BORDER
    };
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(if selected { 1.7 } else { 1.0 }, border_color),
        StrokeKind::Inside,
    );
    let clipped = painter.with_clip_rect(rect.shrink(1.0).intersect(painter.clip_rect()));
    let color = theme::block_color(card.block);

    if detail == CardDetail::Overview {
        // At 25% this is a small diagram tile, not a miniature form. Its one
        // readable title has the whole tile to itself; values live in Inspector.
        let accent = Rect::from_min_max(
            rect.left_top() + vec2(4.0, 4.0),
            rect.right_top() + vec2(-4.0, 6.0),
        );
        clipped.rect_filled(accent, 1.0, color.gamma_multiply(0.65));
        label(
            &clipped,
            rect.shrink2(vec2(5.0, 10.0)),
            &card.name,
            FontId::new((16.0 * z).max(7.5), theme::bold_family()),
            theme::TEXT,
            egui::Align::Center,
        );
    } else {
        let full = detail == CardDetail::Full;
        let icon = Rect::from_min_size(local(CARD_PADDING, 16.0), Vec2::splat(28.0 * z));
        clipped.rect_filled(icon, 7.0 * z, color.gamma_multiply(0.10));
        theme::block_icon(&clipped, icon, card.block, color);
        let title_right = if full && card.kind == NodeKind::Component {
            WIDTH - 48.0
        } else {
            WIDTH - CARD_PADDING
        };
        if full {
            label(
                &clipped,
                cell(56.0, 12.0, title_right, 28.0),
                &card.caption,
                FontId::proportional(10.5 * z),
                theme::MUTED,
                egui::Align::Min,
            );
        }
        label(
            &clipped,
            cell(56.0, if full { 28.0 } else { 14.0 }, title_right, 56.0),
            &card.name,
            FontId::new(16.0 * z, theme::bold_family()),
            theme::TEXT,
            egui::Align::Min,
        );
        if full && card.kind == NodeKind::Component {
            let button = camera.rect(card.open_button());
            Icon::ExternalLink.paint(&clipped, button.shrink(6.0 * z), theme::MUTED);
        }
        if let Some(subtitle) = &card.subtitle {
            label(
                &clipped,
                cell(CARD_PADDING, 64.0, WIDTH - CARD_PADDING, 88.0),
                subtitle,
                FontId::proportional(12.0 * z),
                theme::LINK,
                egui::Align::Min,
            );
        }
        for (index, row) in card.inputs.iter().enumerate() {
            let top = card.input_row_top(index);
            if full {
                let name = if row.required {
                    format!("{} *", row.label)
                } else {
                    row.label.clone()
                };
                label(
                    &clipped,
                    cell(CARD_PADDING, top, WIDTH - CARD_PADDING, top + 18.0),
                    &name,
                    FontId::proportional(12.0 * z),
                    theme::MUTED,
                    egui::Align::Min,
                );
            }
            let field = camera.rect(card.input_field(index));
            let (text, text_color, framed) = match &row.value {
                RowValue::Inline { .. } | RowValue::TypeChoice(_)
                    if z >= super::INLINE_EDIT_ZOOM =>
                {
                    (String::new(), theme::TEXT, true)
                }
                RowValue::Inline { text, .. } | RowValue::TypeChoice(text) => {
                    (text.clone(), theme::TEXT, true)
                }
                RowValue::Source(text) => (text.clone(), theme::LINK, false),
                RowValue::Summary(text) => (text.clone(), theme::TEXT, false),
                RowValue::Missing => ("Value missing".into(), theme::DANGER, false),
                RowValue::Optional => ("Optional".into(), theme::MUTED, false),
            };
            clipped.rect_filled(field, 5.0 * z, theme::FIELD);
            if framed {
                clipped.rect_stroke(
                    field,
                    5.0 * z,
                    Stroke::new(0.75, theme::BORDER),
                    StrokeKind::Inside,
                );
            }
            if !text.is_empty() {
                label(
                    &clipped,
                    field.shrink2(vec2(8.0 * z, 2.0 * z)),
                    &text,
                    FontId::proportional(13.0 * z),
                    text_color,
                    egui::Align::Min,
                );
            }
        }
        let divider = card.outputs_divider();
        clipped.line_segment(
            [
                local(CARD_PADDING, divider),
                local(WIDTH - CARD_PADDING, divider),
            ],
            Stroke::new(0.75, theme::BORDER),
        );
        for (index, row) in card.outputs.iter().enumerate() {
            let y = card.output_y(index);
            let type_width = if full {
                clipped
                    .layout_no_wrap(
                        row.type_label.clone(),
                        FontId::proportional(11.0 * z),
                        theme::MUTED,
                    )
                    .size()
                    .x
                    .min(76.0 * z)
            } else {
                0.0
            };
            let right = WIDTH - CARD_PADDING;
            if full {
                label(
                    &clipped,
                    cell(right - type_width / z, y - 12.0, right, y + 12.0),
                    &row.type_label,
                    FontId::proportional(11.0 * z),
                    theme::MUTED,
                    egui::Align::Max,
                );
            }
            label(
                &clipped,
                cell(
                    CARD_PADDING,
                    y - 12.0,
                    right - if full { type_width / z + 12.0 } else { 0.0 },
                    y + 12.0,
                ),
                &row.label,
                FontId::proportional(12.0 * z),
                theme::TEXT,
                egui::Align::Max,
            );
        }
        let footer = camera.rect(card.footer());
        clipped.line_segment(
            [footer.left_top(), footer.right_top()],
            Stroke::new(0.75, theme::BORDER),
        );
        if full {
            let action = camera.rect(card.action_button());
            let status_rect = Rect::from_min_max(
                footer.left_top() + vec2(CARD_PADDING * z, 0.0),
                egui::pos2(action.left() - 8.0 * z, footer.bottom()),
            );
            let (status, color) = step.map_or(("Ready".to_owned(), theme::MUTED), |s| {
                (
                    format!("{} · {} ms", s.status.label(), s.duration_ms),
                    status_color(s.status),
                )
            });
            label(
                &clipped,
                status_rect,
                &status,
                FontId::proportional(11.0 * z),
                color,
                egui::Align::Min,
            );
            Icon::Plus.paint(&clipped, action.shrink(5.0 * z), theme::MUTED);
            for (index, (port, text)) in card.controls.iter().enumerate() {
                let button = camera.rect(card.control_button(index));
                let color = match *port {
                    apiforge_core::graph::TRUE_PORT => theme::BRANCH_YES,
                    apiforge_core::graph::FALSE_PORT => theme::BRANCH_NO,
                    _ => theme::MUTED,
                };
                label(
                    &clipped,
                    Rect::from_min_max(
                        button.min + vec2(24.0 * z, 2.0 * z),
                        button.max - vec2(24.0 * z, 2.0 * z),
                    ),
                    text.trim_end_matches(" →"),
                    FontId::proportional(11.0 * z),
                    color,
                    egui::Align::Center,
                );
            }
        } else if let Some(step) = step {
            clipped.circle_filled(
                footer.left_center() + vec2(CARD_PADDING * z, 0.0),
                2.0,
                status_color(step.status),
            );
        }
    }

    // Connections remain visible and usable at every detail level.
    for index in 0..card.inputs.len() {
        socket(painter, camera.to_screen(card.row_anchor(index)), false, z);
    }
    for row in &card.outputs {
        if let Some(anchor) = card.output_anchor(&row.name) {
            socket(painter, camera.to_screen(anchor), false, z);
        }
    }
    for (port, _) in &card.controls {
        if let Some(anchor) = card.output_anchor(port) {
            socket(painter, camera.to_screen(anchor), true, z);
        }
    }
    if let Some(anchor) = card.input_anchor(apiforge_core::graph::ORDER_PORT, &[]) {
        socket(painter, camera.to_screen(anchor), true, z);
    }
}

/// Data sockets are round, control sockets square, so they differ beyond colour.
fn socket(painter: &Painter, center: Pos2, control: bool, zoom: f32) {
    let radius = (4.5 * zoom).max(1.25);
    let stroke = Stroke::new((1.0 * zoom).clamp(0.6, 1.0), theme::SOCKET);
    if control {
        painter.rect(
            Rect::from_center_size(center, Vec2::splat(radius * 2.0)),
            1.5,
            theme::WHITE,
            stroke,
            StrokeKind::Inside,
        );
    } else {
        painter.circle(center, radius, theme::WHITE, stroke);
    }
}

pub const SHORTCUT_HINT_TEXT: &str =
    "Select: drag or Ctrl+click  ·  Pan: right-drag  ·  Zoom: Ctrl+scroll";

pub fn mode_line_text(graph: &Graph) -> &'static str {
    let has_start = graph
        .nodes
        .iter()
        .any(|node| node.kind == NodeKind::Trigger);
    if has_start {
        "Execution follows Start · Inactive branches are skipped."
    } else {
        "All steps run · Add Start to choose an execution path."
    }
}

pub fn overlay_chrome(painter: &Painter, overlays: &super::layout::BoardOverlays, graph: &Graph) {
    painter.rect_filled(overlays.header, 0.0, theme::BG);
    painter.line_segment(
        [
            overlays.header.left_bottom(),
            overlays.header.right_bottom(),
        ],
        Stroke::new(1.0, theme::BORDER),
    );
    painter.rect_filled(overlays.footer, 0.0, theme::BG);
    painter.line_segment(
        [overlays.footer.left_top(), overlays.footer.right_top()],
        Stroke::new(1.0, theme::BORDER),
    );

    let starts = graph
        .nodes
        .iter()
        .any(|node| node.kind == NodeKind::Trigger);
    let caption = format!(
        "{} steps  ·  {}",
        graph.nodes.len(),
        if starts { "From Start" } else { "All steps" }
    );
    label(
        painter,
        overlays.banner,
        &caption,
        FontId::proportional(theme::FONT_META),
        theme::MUTED,
        egui::Align::Min,
    );

    if let super::layout::HintPlacement::Inline(rect) = overlays.hint {
        painter.text(
            rect.left_center(),
            Align2::LEFT_CENTER,
            SHORTCUT_HINT_TEXT,
            FontId::proportional(theme::FONT_META),
            theme::MUTED,
        );
    }
}

pub fn minimap(
    painter: &Painter,
    minimap_rect: Option<Rect>,
    board_rect: Rect,
    cards: &[Card],
    camera: Camera,
    selection: &HashSet<&str>,
) -> Option<(Rect, Rect, f32)> {
    let rect = minimap_rect?;
    let bounds = cards.iter().map(Card::rect).reduce(Rect::union)?;
    painter.rect(
        rect,
        7.0,
        theme::WHITE,
        Stroke::new(1.0, theme::BORDER),
        StrokeKind::Inside,
    );
    let inner = rect.shrink(8.0);
    let scale =
        (inner.width() / bounds.width().max(1.0)).min(inner.height() / bounds.height().max(1.0));
    let map = |point: Pos2| inner.min + (point - bounds.min) * scale;
    for card in cards {
        let fill = if selection.contains(card.id.as_str()) {
            theme::ACCENT
        } else {
            theme::MINIMAP
        };
        painter.rect_filled(
            Rect::from_min_max(map(card.rect().min), map(card.rect().max)),
            1.0,
            fill,
        );
    }
    let visible = Rect::from_min_max(
        camera.to_world(board_rect.min),
        camera.to_world(board_rect.max),
    );
    painter.rect_stroke(
        Rect::from_min_max(map(visible.min), map(visible.max)).intersect(rect),
        1.0,
        Stroke::new(1.0, theme::MUTED),
        StrokeKind::Inside,
    );
    Some((inner, bounds, scale))
}

#[cfg(test)]
mod tests {
    use super::super::layout::CardDetail;
    use super::*;

    #[test]
    fn every_card_label_stays_in_its_cell_from_25_to_160_percent() {
        let context = egui::Context::default();
        theme::apply(&context);
        let project = apiforge_core::demos::shop_project();
        let graph = &project.scenarios[0].graph;
        for zoom in [0.25, 0.35, 0.45, 0.6, 0.75, 1.0, 1.6] {
            for node in &graph.nodes {
                let mut card = Card::build(node, graph, &project);
                card.origin = Pos2::ZERO;
                // Unbroken identifiers and long captions must ellipsise as well.
                card.name = "ŻółćVeryLongUnbrokenComponentName".repeat(5);
                card.caption = "Component · a deliberately long caption".repeat(4);
                for row in &mut card.inputs {
                    row.label = "Extremely long argument label".repeat(4);
                    row.value = RowValue::Source("long_reference_without_spaces".repeat(5));
                }
                for row in &mut card.outputs {
                    row.label = "long_output_name_without_spaces".repeat(4);
                    row.type_label = "complicated_type".repeat(4);
                }
                let camera = Camera {
                    zoom,
                    pan: vec2(96.0, 96.0),
                };
                let bounds = camera.rect(card.rect());
                let mut output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 1200.0))),
                        ..Default::default()
                    },
                    |ui| super::card(ui.painter(), &card, camera, true, None),
                );
                let mut labels = 0;
                for shape in &output.shapes {
                    if let Shape::Text(text) = &shape.shape {
                        labels += 1;
                        let text_bounds = text.galley.rect.translate(text.pos.to_vec2());
                        assert!(
                            bounds.expand(0.75).contains_rect(shape.clip_rect),
                            "label clip escapes card at {zoom}: {:?}",
                            shape.clip_rect
                        );
                        assert!(
                            shape.clip_rect.expand(1.0).contains_rect(text_bounds),
                            "oversized label at {zoom}: {:?}, clip {:?}, text {:?}",
                            text_bounds,
                            shape.clip_rect,
                            text.galley.job.text
                        );
                    }
                }
                assert!(labels > 0);
                if CardDetail::at_zoom(zoom) == CardDetail::Overview {
                    assert_eq!(
                        labels, 1,
                        "overview must not retain overlapping micro-form labels"
                    );
                }
                output.textures_delta.clear();
            }
        }
    }
}

//! Drawing of the board. Nothing here handles input.

use super::{
    Camera,
    layout::{Card, RowValue, WIDTH},
};
use crate::theme;
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
    let spacing = (22.0 * camera.zoom).max(12.0);
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
            painter.circle_filled(egui::pos2(x, y), 0.8, theme::GRID);
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
    if control && let Some(branch) = branch {
        painter.text(
            bezier(points, 0.5) + vec2(6.0, 0.0),
            Align2::LEFT_CENTER,
            match branch {
                Branch::True => "yes",
                Branch::False => "no",
            },
            FontId::proportional(10.0 * zoom.max(0.7)),
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
    let z = camera.zoom;
    let rect = camera.rect(card.rect());
    let local = |x: f32, y: f32| camera.to_screen(card.origin + vec2(x, y));
    let radius = 10.0 * z;
    painter.rect_filled(
        rect.translate(vec2(0.0, 3.0 * z)),
        radius,
        Color32::from_black_alpha(14),
    );
    painter.rect_filled(rect, radius, theme::WHITE);
    let border = match (selected, step) {
        (true, _) => Stroke::new(2.0, theme::ACCENT),
        (false, Some(step)) => Stroke::new(1.4, status_color(step.status).gamma_multiply(0.7)),
        (false, None) => Stroke::new(1.0, theme::BORDER),
    };
    painter.rect_stroke(rect, radius, border, StrokeKind::Inside);
    let clipped = painter.with_clip_rect(rect.shrink(2.0).intersect(painter.clip_rect()));

    let color = theme::kind_color(card.kind);
    let icon = Rect::from_min_size(local(14.0, 14.0), Vec2::splat(32.0 * z));
    clipped.rect_filled(icon, 7.0 * z, color.gamma_multiply(0.12));
    theme::kind_icon(&clipped, icon, card.kind, color);
    let title_right = if card.kind == NodeKind::Component {
        WIDTH - 46.0
    } else {
        WIDTH - 12.0
    };
    clipped.text(
        local(56.0, 21.0),
        Align2::LEFT_CENTER,
        &card.caption,
        FontId::proportional((11.0 * z).max(10.0)),
        theme::MUTED,
    );
    clipped
        .with_clip_rect(
            Rect::from_min_max(local(56.0, 28.0), local(title_right, 52.0))
                .intersect(clipped.clip_rect()),
        )
        .text(
            local(56.0, 40.0),
            Align2::LEFT_CENTER,
            &card.name,
            FontId::new((15.0 * z).max(13.0), theme::bold_family()),
            theme::TEXT,
        );
    if card.kind == NodeKind::Component {
        let button = camera.rect(card.open_button());
        clipped.rect_stroke(
            button,
            5.0 * z,
            Stroke::new(1.0, theme::BORDER),
            StrokeKind::Inside,
        );
        clipped.text(
            button.center(),
            Align2::CENTER_CENTER,
            "↗",
            FontId::proportional(14.0 * z),
            theme::MUTED,
        );
    }
    if let Some(subtitle) = &card.subtitle {
        let at = camera.to_screen(card.subtitle_center());
        clipped
            .with_clip_rect(
                Rect::from_min_max(local(12.0, 56.0), local(WIDTH - 12.0, 82.0))
                    .intersect(clipped.clip_rect()),
            )
            .text(
                at,
                Align2::LEFT_CENTER,
                subtitle,
                FontId::proportional((12.0 * z).max(11.0)),
                theme::LINK,
            );
    }

    for (index, row) in card.inputs.iter().enumerate() {
        let top = card.input_row_top(index);
        let label = if row.required {
            format!("{} *", row.label)
        } else {
            row.label.clone()
        };
        clipped.text(
            local(14.0, top + 10.0),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional((11.5 * z).max(10.5)),
            theme::MUTED,
        );
        let field = camera.rect(card.input_field(index));
        let (text, text_color, framed) = match &row.value {
            RowValue::Inline { .. } | RowValue::TypeChoice(_) if z >= super::INLINE_EDIT_ZOOM => {
                (String::new(), theme::TEXT, true)
            }
            RowValue::Inline { text, .. } => (text.clone(), theme::TEXT, true),
            RowValue::TypeChoice(text) => (text.clone(), theme::TEXT, true),
            RowValue::Source(text) => (text.clone(), theme::LINK, false),
            RowValue::Summary(text) => (text.clone(), theme::TEXT, false),
            RowValue::Missing => ("Value missing".into(), theme::DANGER, false),
            RowValue::Optional => ("optional".into(), theme::MUTED, false),
        };
        clipped.rect_filled(field, 4.0 * z, theme::FIELD);
        if framed {
            clipped.rect_stroke(
                field,
                4.0 * z,
                Stroke::new(0.8, theme::BORDER),
                StrokeKind::Inside,
            );
        }
        if !text.is_empty() {
            clipped
                .with_clip_rect(field.shrink(3.0).intersect(clipped.clip_rect()))
                .text(
                    field.left_center() + vec2(6.0 * z, 0.0),
                    Align2::LEFT_CENTER,
                    text,
                    FontId::proportional((12.0 * z).max(11.0)),
                    text_color,
                );
        }
        if let Some(anchor) = card.input_anchor(&row.name) {
            socket(painter, camera.to_screen(anchor), false, z);
        }
    }

    let divider = card.outputs_divider();
    clipped.line_segment(
        [local(0.0, divider), local(WIDTH, divider)],
        Stroke::new(0.6, theme::BORDER),
    );
    for (index, row) in card.outputs.iter().enumerate() {
        let y = card.output_y(index);
        let type_width = clipped
            .layout_no_wrap(
                row.type_label.clone(),
                FontId::proportional((10.5 * z).max(9.5)),
                theme::MUTED,
            )
            .size()
            .x;
        clipped.text(
            local(WIDTH - 16.0, y),
            Align2::RIGHT_CENTER,
            &row.type_label,
            FontId::proportional((10.5 * z).max(9.5)),
            theme::MUTED,
        );
        clipped.text(
            local(WIDTH - 16.0, y) - vec2(type_width + 8.0 * z, 0.0),
            Align2::RIGHT_CENTER,
            &row.label,
            FontId::proportional((11.5 * z).max(10.5)),
            theme::TEXT,
        );
        if let Some(anchor) = card.output_anchor(&row.name) {
            socket(painter, camera.to_screen(anchor), false, z);
        }
    }

    let footer = camera.rect(card.footer());
    clipped.line_segment(
        [footer.left_top(), footer.right_top()],
        Stroke::new(0.6, theme::BORDER),
    );
    match step {
        Some(step) => {
            clipped.text(
                footer.left_center() + vec2(14.0 * z, 0.0),
                Align2::LEFT_CENTER,
                format!("● {}  {} ms", step.status.label(), step.duration_ms),
                FontId::proportional((11.5 * z).max(10.5)),
                status_color(step.status),
            );
        }
        None => {
            clipped.text(
                footer.left_center() + vec2(14.0 * z, 0.0),
                Align2::LEFT_CENTER,
                "Ready",
                FontId::proportional((11.0 * z).max(10.0)),
                theme::MUTED,
            );
        }
    }
    let action = camera.rect(card.action_button());
    clipped.rect_stroke(
        action,
        5.0 * z,
        Stroke::new(1.0, theme::BORDER),
        StrokeKind::Inside,
    );
    clipped.text(
        action.center(),
        Align2::CENTER_CENTER,
        "+ Action",
        FontId::proportional((11.5 * z).max(10.5)),
        theme::TEXT,
    );

    for (index, (port, label)) in card.controls.iter().enumerate() {
        let button = camera.rect(card.control_button(index));
        clipped.line_segment(
            [button.left_top(), button.right_top()],
            Stroke::new(0.6, theme::BORDER),
        );
        let color = match *port {
            apiforge_core::graph::TRUE_PORT => theme::BRANCH_YES,
            apiforge_core::graph::FALSE_PORT => theme::BRANCH_NO,
            _ => theme::MUTED,
        };
        clipped.text(
            button.center() - vec2(0.0, 2.0 * z),
            Align2::CENTER_CENTER,
            *label,
            FontId::proportional((11.5 * z).max(10.5)),
            color,
        );
        if let Some(anchor) = card.output_anchor(port) {
            socket(painter, camera.to_screen(anchor), true, z);
        }
    }
    if let Some(anchor) = card.input_anchor(apiforge_core::graph::ORDER_PORT) {
        socket(painter, camera.to_screen(anchor), true, z);
    }
}

/// Data sockets are round, control sockets square, so they differ beyond colour.
fn socket(painter: &Painter, center: Pos2, control: bool, zoom: f32) {
    let radius = (4.5 * zoom).max(3.0);
    let stroke = Stroke::new(1.2, theme::SOCKET);
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

pub const SHORTCUT_HINT_TEXT: &str = "Ctrl+click: select several  ·  Drag the background: select  ·  Right-drag: pan  ·  Ctrl+wheel: zoom";

pub fn mode_line_text(graph: &Graph) -> &'static str {
    let has_start = graph
        .nodes
        .iter()
        .any(|node| node.kind == NodeKind::Trigger);
    if has_start {
        "Start → steps run in order. Inactive branches are skipped."
    } else {
        "Legacy mode: every step runs. Add a Start and connect the execution path."
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

    painter.text(
        overlays.banner.left_top() + vec2(0.0, 7.0),
        Align2::LEFT_CENTER,
        format!("●  BOARD   ·   {} steps", graph.nodes.len()),
        FontId::proportional(10.0),
        theme::MUTED,
    );
    let mut job = egui::text::LayoutJob::simple(
        mode_line_text(graph).to_owned(),
        FontId::proportional(10.0),
        theme::MUTED,
        overlays.banner_wrap_width,
    );
    job.wrap.break_anywhere = false;
    let galley = painter.layout_job(job);
    painter.galley(
        overlays.banner.left_top() + vec2(0.0, 17.0),
        galley,
        theme::MUTED,
    );

    if let super::layout::HintPlacement::Inline(rect) = overlays.hint {
        painter.text(
            rect.left_center(),
            Align2::LEFT_CENTER,
            SHORTCUT_HINT_TEXT,
            FontId::proportional(10.0),
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

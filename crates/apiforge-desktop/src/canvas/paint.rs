//! Drawing of the board. Nothing here handles input.

use super::{
    Camera,
    layout::{CHIP_HEIGHT, Card, Chip, RowValue, WIDTH},
    routing::Routed,
};
use crate::{icons::Icon, theme, widgets::paint_label as label};
use apiforge_core::{
    format::Graph,
    model::{Branch, NodeKind},
    run::{StepResult, StepStatus},
};
use eframe::egui::{
    self, Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2,
    epaint::CubicBezierShape, pos2, vec2,
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

/// Draws a routed wire: its polyline or rounded corners, the bridges where it
/// crosses a wire routed before it, the arrowhead, and a branch label.
///
/// A bridge is a gap in the line with a small semicircle over it, so a crossing
/// reads as one wire passing above the other instead of a junction.
pub fn routed_wire(painter: &Painter, routed: &Routed, camera: Camera, selected: bool) {
    let mut color = wire_color(selected, routed.control, routed.branch);
    if !routed.routed {
        // A wire drawn directly during a drag is lighter, so the board shows that
        // the route is provisional until the next idle frame.
        color = color.gamma_multiply(0.55);
    }
    let width = if selected { 2.4 } else { 1.6 };
    let zoom = camera.zoom;
    let line: Vec<Pos2> = if routed.pieces.is_empty() {
        // A direct curve: the wire has no route, or the board is mid-drag.
        let [start, end] = [
            routed.points.first().copied().unwrap_or_default(),
            routed.points.last().copied().unwrap_or_default(),
        ]
        .map(|point| camera.to_screen(point));
        let points = wire_points(start, end, routed.control, zoom);
        (0..=32)
            .map(|step| bezier(points, step as f32 / 32.0))
            .collect()
    } else {
        flatten(&routed.pieces, camera)
    };
    if line.len() < 2 {
        return;
    }
    let hops: Vec<Pos2> = routed
        .bridges
        .iter()
        .map(|(at, _)| camera.to_screen(*at))
        .collect();
    let radius = (5.0 * zoom).clamp(2.0, 7.0);
    let stroke = Stroke::new(width, color);
    for run in split_at_bridges(&line, &hops, radius) {
        match run {
            Run::Line(points) if routed.control => dashes(painter, &points, stroke, zoom),
            Run::Line(points) => {
                painter.add(Shape::line(points, stroke));
            }
            Run::Hop { at, direction } => {
                let normal = vec2(-direction.y, direction.x);
                let arc: Vec<Pos2> = (0..=8)
                    .map(|step| {
                        let angle = std::f32::consts::PI * step as f32 / 8.0;
                        at - direction * radius * angle.cos() + normal * radius * angle.sin()
                    })
                    .collect();
                painter.add(Shape::line(arc, stroke));
            }
        }
    }
    let end = line[line.len() - 1];
    let direction = (end - line[line.len() - 2]).normalized();
    arrowhead(painter, end, direction, color, zoom);
    if routed.control
        && zoom >= 0.6
        && let Some(branch) = routed.branch
    {
        let middle = line[line.len() / 2];
        painter.text(
            middle + vec2(6.0, 0.0),
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

fn wire_color(selected: bool, control: bool, branch: Option<Branch>) -> Color32 {
    match (selected, control, branch) {
        (true, ..) => theme::ACCENT,
        (_, true, Some(Branch::False)) => theme::BRANCH_NO,
        (_, true, Some(Branch::True)) => theme::BRANCH_YES,
        (_, true, None) => theme::CONTROL_WIRE,
        (_, false, _) => theme::DATA_WIRE,
    }
}

fn arrowhead(painter: &Painter, end: Pos2, direction: Vec2, color: Color32, zoom: f32) {
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
}

/// A route's pieces as one screen-space polyline; curves are sampled.
fn flatten(pieces: &[apiforge_layout::Piece], camera: Camera) -> Vec<Pos2> {
    let point = |p: apiforge_layout::Point| camera.to_screen(pos2(p.x, p.y));
    let mut line: Vec<Pos2> = Vec::with_capacity(pieces.len() * 4);
    for piece in pieces {
        let start = point(piece.from());
        if line.last().is_none_or(|last| last.distance(start) > 0.01) {
            line.push(start);
        }
        match piece {
            apiforge_layout::Piece::Line { to, .. } => line.push(point(*to)),
            apiforge_layout::Piece::Curve {
                from,
                control1,
                control2,
                to,
            } => {
                let control = [*from, *control1, *control2, *to].map(point);
                line.extend((1..=6).map(|step| bezier(control, step as f32 / 6.0)));
            }
        }
    }
    line
}

/// A piece of a drawn wire: a run of line, or a bridge over a crossing.
enum Run {
    Line(Vec<Pos2>),
    Hop { at: Pos2, direction: Vec2 },
}

/// Splits a polyline where a bridge sits, leaving a gap of `radius` on each side.
fn split_at_bridges(line: &[Pos2], hops: &[Pos2], radius: f32) -> Vec<Run> {
    let mut runs = Vec::new();
    let mut pending: Vec<Pos2> = Vec::new();
    for pair in line.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        let length = from.distance(to);
        if length <= f32::EPSILON {
            continue;
        }
        let direction = (to - from) / length;
        let mut on_segment: Vec<(f32, Pos2)> = hops
            .iter()
            .filter_map(|hop| {
                let along = (*hop - from).dot(direction);
                let across = (*hop - (from + direction * along)).length();
                (across < 0.5 && along > radius && along < length - radius).then_some((along, *hop))
            })
            .collect();
        on_segment.sort_by(|left, right| left.0.total_cmp(&right.0));
        pending.push(from);
        for (along, hop) in on_segment {
            pending.push(from + direction * (along - radius));
            runs.push(Run::Line(std::mem::take(&mut pending)));
            runs.push(Run::Hop { at: hop, direction });
            pending.push(from + direction * (along + radius));
        }
        pending.push(to);
        // Keep the run going into the next segment, so a corner stays joined.
        pending.dedup_by(|left, right| left.distance(*right) < 0.01);
    }
    if pending.len() > 1 {
        runs.push(Run::Line(pending));
    }
    runs
}

/// Draws a polyline as dashes, which is how an ordering wire is shown.
fn dashes(painter: &Painter, line: &[Pos2], stroke: Stroke, zoom: f32) {
    let on = 7.0 * zoom.max(0.5);
    let off = 5.0 * zoom.max(0.5);
    let mut travelled = 0.0;
    for pair in line.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        let length = from.distance(to);
        if length <= f32::EPSILON {
            continue;
        }
        let direction = (to - from) / length;
        let mut position = 0.0;
        while position < length {
            let phase = (travelled + position) % (on + off);
            let remaining = if phase < on {
                on - phase
            } else {
                on + off - phase
            };
            let step = remaining.min(length - position);
            if phase < on {
                painter.line_segment(
                    [
                        from + direction * position,
                        from + direction * (position + step),
                    ],
                    stroke,
                );
            }
            position += step.max(0.5);
        }
        travelled += length;
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
    // Part of what a selected proxy stands for, or reads it.
    related: bool,
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
    } else if related {
        theme::VARIABLE
    } else {
        theme::BORDER
    };
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(if selected || related { 1.7 } else { 1.0 }, border_color),
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

/// A proxy chip: the name of the value, and the socket its wires leave from.
///
/// A chip is deliberately not a card. It holds no arguments and no settings — it
/// is the same value shown in another place — so it stays a pill with one name.
pub fn chip(painter: &Painter, chip: &Chip, camera: Camera, selected: bool) {
    let z = camera.zoom;
    let rect = camera.rect(chip.rect());
    let radius = (CHIP_HEIGHT / 2.0 * z).max(2.0);
    let color = if chip.known {
        theme::VARIABLE
    } else {
        theme::MUTED
    };
    painter.rect_filled(
        rect.translate(vec2(0.0, 1.5 * z)),
        radius,
        Color32::from_black_alpha(8),
    );
    painter.rect(
        rect,
        radius,
        color.gamma_multiply(if selected { 0.22 } else { 0.12 }),
        Stroke::new(
            if selected { 1.7 } else { 1.0 },
            if selected {
                theme::ACCENT
            } else {
                color.gamma_multiply(0.45)
            },
        ),
        StrokeKind::Inside,
    );
    let clipped = painter.with_clip_rect(rect.shrink(1.0).intersect(painter.clip_rect()));
    let local = |x: f32, y: f32| camera.to_screen(chip.origin + vec2(x, y));
    let icon = Rect::from_min_size(local(8.0, 9.0), Vec2::splat(18.0 * z));
    Icon::Variable.paint(&clipped, icon, color);
    let follows = chip.source_node().is_some();
    label(
        &clipped,
        Rect::from_min_max(
            local(30.0, 2.0),
            local(
                chip.width() - if follows { 32.0 } else { 12.0 },
                CHIP_HEIGHT - 2.0,
            ),
        ),
        &chip.label,
        FontId::new((13.0 * z).max(6.0), theme::bold_family()),
        if chip.known {
            theme::TEXT
        } else {
            theme::MUTED
        },
        egui::Align::Min,
    );
    if follows && super::layout::CardDetail::at_zoom(z) == super::layout::CardDetail::Full {
        Icon::ExternalLink.paint(
            &clipped,
            camera.rect(chip.follow_button()).shrink(4.0 * z),
            theme::MUTED,
        );
    }
    socket(painter, camera.to_screen(chip.output_anchor()), false, z);
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
    let has_start = graph.nodes.iter().any(|node| node.kind == NodeKind::Start);
    if has_start {
        "Execution follows Start · Inactive branches are skipped."
    } else {
        "All steps run · Add Start to choose an execution path."
    }
}

/// The steps summary of the board's header row, e.g. "6 steps  ·  All steps".
pub fn steps_summary(graph: &Graph) -> String {
    let starts = graph.nodes.iter().any(|node| node.kind == NodeKind::Start);
    format!(
        "{} steps  ·  {}",
        graph.nodes.len(),
        if starts { "From Start" } else { "All steps" }
    )
}

/// The font of the steps summary.
pub fn summary_font() -> FontId {
    FontId::proportional(theme::FONT_META)
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

    label(
        painter,
        overlays.banner,
        &steps_summary(graph),
        summary_font(),
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
    shapes: super::Shapes<'_>,
    camera: Camera,
    selection: &HashSet<&str>,
) -> Option<(Rect, Rect, f32)> {
    let rect = minimap_rect?;
    let bounds = shapes.bounds()?;
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
    for card in shapes.cards {
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
    for chip in shapes.chips {
        let fill = if selection.contains(chip.id()) {
            theme::ACCENT
        } else {
            theme::VARIABLE.gamma_multiply(0.6)
        };
        painter.rect_filled(
            Rect::from_min_max(map(chip.rect().min), map(chip.rect().max)),
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
                    |ui| super::card(ui.painter(), &card, camera, true, false, None),
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

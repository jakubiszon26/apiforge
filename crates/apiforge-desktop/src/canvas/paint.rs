//! Drawing of the board. Nothing here handles input.

use super::{
    Camera,
    collapse::Aggregate,
    layout::{CHIP_HEIGHT, COMPACT_HEADER, Card, CardDetail, Chip, CompactRole, RowValue, WIDTH},
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
use std::{borrow::Cow, collections::HashSet};

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

/// The name on an `@after` drawn from a folded check's subject (LAY-11):
/// `after cart_total`, on a light ground halfway along the wire.
pub fn wire_label(painter: &Painter, routed: &Routed, camera: Camera, text: &str) {
    if routed.points.len() < 2 {
        return;
    }
    let line: Vec<Pos2> = routed
        .points
        .iter()
        .map(|point| camera.to_screen(*point))
        .collect();
    let length: f32 = line.windows(2).map(|pair| pair[0].distance(pair[1])).sum();
    let mut left = length / 2.0;
    let mut middle = line[0];
    for pair in line.windows(2) {
        let piece = pair[0].distance(pair[1]);
        if piece >= left {
            middle = pair[0] + (pair[1] - pair[0]) * (left / piece.max(f32::EPSILON));
            break;
        }
        left -= piece;
    }
    let galley = painter.layout_no_wrap(
        text.to_owned(),
        FontId::proportional((11.0 * camera.zoom).max(8.0)),
        theme::MUTED,
    );
    let rect = Rect::from_center_size(middle, galley.size() + vec2(6.0, 2.0));
    painter.rect_filled(rect, 3.0, theme::WHITE.gamma_multiply(0.9));
    painter.galley(rect.min + vec2(3.0, 1.0), galley, theme::MUTED);
}

/// The smallest text a card writes on screen, in points, by what it says
/// (LAY-09): a name, a step number and a line.
pub const NAME_MIN: f32 = 11.0;
// LAY-07's badge writes the number; until then only the tests read these.
#[cfg_attr(not(test), allow(dead_code))]
pub const NUMBER_MIN: f32 = 9.0;
pub const LINE_MIN: f32 = 9.0;
/// The sizes at 100 %: a card's name and a line.
const NAME_TEXT: f32 = 16.0;
const LINE_TEXT: f32 = 12.0;
/// Below this zoom a card is its band, its border and its badge; its name
/// is written only where it fits (LAY-09).
const LABELS_FROM: f32 = 0.25;

/// A text's size on screen at `zoom`, never under `min`: the one rule every
/// card kind follows at every level, so no text gets smaller when zooming in.
pub fn min_on_screen(size: f32, zoom: f32, min: f32) -> f32 {
    (size * zoom).max(min)
}

/// What a text on a card says, for the minimum sizes (LAY-09).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextKind {
    Name,
    /// A step number (LAY-07).
    #[cfg_attr(not(test), allow(dead_code))]
    Number,
    /// The execution line, the checks line or the checks pill.
    Result,
    Subtitle,
    /// Any other line: an input, an output, a caption.
    Line,
}

/// A text a card drew: what it says, its size on screen and its colour.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawnText {
    pub kind: TextKind,
    pub size: f32,
    pub color: Color32,
    pub text: String,
}

/// What a card drew in a frame, which only a test build collects.
#[derive(Clone, Debug, Default)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct Drawn {
    /// The lines of a compact card, in order.
    pub lines: Vec<String>,
    pub texts: Vec<DrawnText>,
    /// The 2 px border of a failed or blocked card.
    pub signal: Option<Color32>,
    /// The band's colour.
    pub band: Option<Color32>,
}

impl Drawn {
    /// Draws a text in `rect` and notes it.
    #[allow(clippy::too_many_arguments)]
    fn label(
        &mut self,
        painter: &Painter,
        rect: Rect,
        text: &str,
        kind: TextKind,
        font: FontId,
        color: Color32,
        align: egui::Align,
    ) {
        #[cfg(test)]
        self.texts.push(DrawnText {
            kind,
            size: font.size,
            color,
            text: text.to_owned(),
        });
        #[cfg(not(test))]
        let _ = kind;
        label(painter, rect, text, font, color, align);
    }
}

/// The colour of a card's signal border: red for a step that failed or one
/// of whose checks failed, amber for a blocked one (Q15 = A).
fn signal(step: Option<&StepResult>, aggregate: Option<&Aggregate>) -> Option<Color32> {
    match step.map(|step| step.status) {
        Some(StepStatus::Failed | StepStatus::Error) => Some(theme::DANGER),
        _ if aggregate.is_some_and(Aggregate::failed) => Some(theme::DANGER),
        Some(StepStatus::Blocked) => Some(theme::WARNING),
        _ => None,
    }
}

/// The execution line of a step that ran: `HTTP 200 · 12 ms` for a request
/// that has a status, `Passed · 3 ms` for another step.
fn execution(step: &StepResult) -> (String, Color32) {
    let what = match step
        .outputs
        .get("status")
        .and_then(serde_json::Value::as_u64)
    {
        Some(code) if step.kind == NodeKind::Request => format!("HTTP {code}"),
        _ => step.status.label().to_owned(),
    };
    (
        format!("{what} · {} ms", step.duration_ms),
        status_color(step.status),
    )
}

/// The checks pill under a card's name at Overview (Q5 = A): what became of
/// the step's checks, red when one failed. Drawn only where it fits.
fn pill(
    painter: &Painter,
    drawn: &mut Drawn,
    card: Rect,
    top: f32,
    // Under a centred name, or at the left under a name at the left.
    centred: bool,
    zoom: f32,
    aggregate: &Aggregate,
) {
    let text = aggregate.text();
    if text.is_empty() || zoom < LABELS_FROM {
        return;
    }
    let font = FontId::proportional(min_on_screen(LINE_TEXT, zoom, LINE_MIN));
    let color = if aggregate.failed() {
        theme::DANGER
    } else {
        theme::MUTED
    };
    let size = painter
        .layout_no_wrap(text.clone(), font.clone(), color)
        .size();
    let width = (size.x + 10.0).min(card.width() - 10.0);
    let left = if centred {
        card.center().x - width / 2.0
    } else {
        card.left() + 5.0
    };
    let rect = Rect::from_min_size(pos2(left, top), vec2(width, size.y + 2.0));
    if rect.bottom() > card.bottom() - 1.0 || rect.width() < 12.0 {
        return;
    }
    painter.rect_filled(
        rect,
        rect.height() / 2.0,
        if aggregate.failed() {
            theme::DANGER.gamma_multiply(0.12)
        } else {
            theme::FIELD
        },
    );
    drawn.label(
        painter,
        rect.shrink2(vec2(5.0, 0.0)),
        &text,
        TextKind::Result,
        font,
        color,
        egui::Align::Min,
    );
}

/// The border of a failed or blocked step: 2 px at every zoom, while the
/// band keeps the step's own result (Q15 = A). On a selected card it runs
/// inside the selection's border, so both show.
fn signal_border(painter: &Painter, rect: Rect, radius: f32, selected: bool, color: Color32) {
    let (rect, radius) = if selected {
        (rect.shrink(2.0), (radius - 2.0).max(1.0))
    } else {
        (rect, radius)
    };
    painter.rect_stroke(rect, radius, Stroke::new(2.0, color), StrokeKind::Inside);
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
    // What became of the checks that read the step (LAY-11).
    aggregate: Option<&Aggregate>,
) -> Drawn {
    use super::layout::CARD_PADDING;
    let mut drawn = Drawn::default();
    let failed_check = aggregate.is_some_and(Aggregate::failed);
    let z = camera.zoom;
    let line = |size: f32| min_on_screen(size, z, LINE_MIN);
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
    let name_font = FontId::new(min_on_screen(NAME_TEXT, z, NAME_MIN), theme::bold_family());

    if detail == CardDetail::Overview {
        // A small diagram tile, not a miniature form. Its one readable title
        // has the tile to itself, the checks pill under it (Q5 = A); values
        // live in the Inspector.
        let accent = Rect::from_min_max(
            rect.left_top() + vec2(4.0, 4.0),
            rect.right_top() + vec2(-4.0, 6.0),
        );
        clipped.rect_filled(accent, 1.0, color.gamma_multiply(0.65));
        let height = clipped
            .layout_no_wrap(card.name.clone(), name_font.clone(), theme::TEXT)
            .size()
            .y;
        let pill_height = aggregate.map_or(0.0, |_| line(LINE_TEXT) * 1.4);
        let area = rect.shrink2(vec2(5.0, 8.0));
        // Below 25 % only where the tile holds the name, two letters wide.
        if z >= LABELS_FROM || (area.height() >= height && area.width() >= 2.0 * NAME_MIN) {
            let name = Rect::from_center_size(
                area.center() - vec2(0.0, pill_height / 2.0),
                vec2(area.width(), height),
            );
            drawn.label(
                &clipped,
                name,
                &card.name,
                TextKind::Name,
                name_font,
                theme::TEXT,
                egui::Align::Center,
            );
            if let Some(aggregate) = aggregate {
                pill(
                    &clipped,
                    &mut drawn,
                    rect,
                    name.bottom() + 1.0,
                    true,
                    z,
                    aggregate,
                );
            }
        }
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
                FontId::proportional(line(10.5)),
                theme::MUTED,
                egui::Align::Min,
            );
        }
        drawn.label(
            &clipped,
            cell(56.0, if full { 28.0 } else { 14.0 }, title_right, 56.0),
            &card.name,
            TextKind::Name,
            name_font.clone(),
            theme::TEXT,
            egui::Align::Min,
        );
        if full && card.kind == NodeKind::Component {
            let button = camera.rect(card.open_button());
            Icon::ExternalLink.paint(&clipped, button.shrink(6.0 * z), theme::MUTED);
        }
        if let Some(subtitle) = &card.subtitle {
            drawn.label(
                &clipped,
                cell(CARD_PADDING, 64.0, WIDTH - CARD_PADDING, 88.0),
                subtitle,
                TextKind::Subtitle,
                FontId::proportional(line(12.0)),
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
                    FontId::proportional(line(12.0)),
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
                RowValue::MissingStep(step) => {
                    (format!("Missing step “{step}”"), theme::DANGER, false)
                }
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
                drawn.label(
                    &clipped,
                    field.shrink2(vec2(8.0 * z, 2.0 * z)),
                    &text,
                    TextKind::Line,
                    FontId::proportional(line(13.0)),
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
                        FontId::proportional(line(11.0)),
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
                    FontId::proportional(line(11.0)),
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
                FontId::proportional(line(12.0)),
                theme::TEXT,
                egui::Align::Max,
            );
        }
        let footer = camera.rect(card.footer());
        clipped.line_segment(
            [footer.left_top(), footer.right_top()],
            Stroke::new(0.75, theme::BORDER),
        );
        // The result line, at Medium and Full alike: no text gets smaller
        // when zooming in (LAY-09).
        let action = camera.rect(card.action_button());
        let status_rect = Rect::from_min_max(
            footer.left_top() + vec2(CARD_PADDING * z, 0.0),
            egui::pos2(
                if full {
                    action.left() - 8.0 * z
                } else {
                    footer.right() - CARD_PADDING * z
                },
                footer.bottom(),
            ),
        );
        {
            let (mut status, mut color) =
                step.map_or(("Ready".to_owned(), theme::MUTED), execution);
            // The checks' result is a second statement after the step's own.
            if let Some(aggregate) = aggregate {
                status = format!("{status} · {}", aggregate.text());
                if failed_check {
                    color = theme::DANGER;
                }
            }
            drawn.label(
                &clipped,
                status_rect,
                &status,
                TextKind::Result,
                FontId::proportional(line(11.0)),
                color,
                egui::Align::Min,
            );
        }
        if full {
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
                    FontId::proportional(line(11.0)),
                    color,
                    egui::Align::Center,
                );
            }
        }
    }
    // Over everything the card wrote, so a label yields to it (LAY-09).
    drawn.signal = signal(step, aggregate);
    if let Some(color) = drawn.signal {
        signal_border(painter, rect, radius, selected, color);
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
    drawn
}

/// A request card at the compact density (LAY-16): the band, the name, one
/// line per row of its box and the sockets of the rows it shows. Below Full
/// an input reads as a summary (`Body · 2 fields`), at Full as its values, in
/// the same box. Returns the lines written, which only a test build collects.
pub fn compact_card(
    painter: &Painter,
    card: &Card,
    camera: Camera,
    selected: bool,
    related: bool,
    step: Option<&StepResult>,
    // What became of the checks that read the step (LAY-11).
    aggregate: Option<&Aggregate>,
) -> Drawn {
    let mut drawn = Drawn::default();
    let Some(compact) = &card.compact else {
        return drawn;
    };
    let failed_check = aggregate.is_some_and(Aggregate::failed);
    let z = camera.zoom;
    let detail = CardDetail::at_zoom(z);
    let full = detail == CardDetail::Full;
    let rect = camera.rect(card.rect());
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
    let local = |x: f32, y: f32| camera.to_screen(card.origin + vec2(x, y));
    let cell = |l: f32, t: f32, r: f32, b: f32| Rect::from_min_max(local(l, t), local(r, b));

    // The band carries the last run's status.
    let band = Rect::from_min_max(
        rect.left_top(),
        pos2(rect.right(), rect.top() + (5.0 * z).max(3.0)),
    );
    let corner = radius as u8;
    let band_color = step.map_or(Color32::from_rgb(214, 214, 214), |step| {
        status_color(step.status)
    });
    clipped.rect_filled(
        band,
        egui::CornerRadius {
            nw: corner,
            ne: corner,
            sw: 0,
            se: 0,
        },
        band_color,
    );
    drawn.band = Some(band_color);
    let color = theme::block_color(card.block);
    let icon = Rect::from_min_size(local(12.0, 10.0), Vec2::splat(22.0 * z));
    clipped.rect_filled(icon, 5.0 * z, color.gamma_multiply(0.10));
    theme::block_icon(&clipped, icon, card.block, color);
    let call = card.kind == NodeKind::Component;
    // The rows are written from Medium on; at Overview the card is its band,
    // its name and its checks pill (LAY-09).
    let legible = detail != CardDetail::Overview;
    let name_font = FontId::new(min_on_screen(NAME_TEXT, z, NAME_MIN), theme::bold_family());
    let right = if call && legible {
        WIDTH - 40.0
    } else {
        WIDTH - 12.0
    };
    let name = if legible {
        Some(cell(42.0, 6.0, right, COMPACT_HEADER))
    } else {
        // As tall as the name needs, under the band; below 25 % only where
        // the card holds it.
        let height = clipped
            .layout_no_wrap(card.name.clone(), name_font.clone(), theme::TEXT)
            .size()
            .y;
        let name = Rect::from_min_max(
            pos2(local(42.0, 0.0).x, band.bottom()),
            pos2(local(right, 0.0).x, band.bottom() + height),
        );
        (z >= LABELS_FROM
            || (name.bottom() <= rect.bottom() - 1.0 && name.width() >= 2.0 * NAME_MIN))
            .then_some(name)
    };
    if let Some(name) = name {
        drawn.label(
            &clipped,
            name,
            &card.name,
            TextKind::Name,
            name_font,
            theme::TEXT,
            egui::Align::Min,
        );
        if !legible && let Some(aggregate) = aggregate {
            pill(
                &clipped,
                &mut drawn,
                rect,
                name.bottom() + 1.0,
                false,
                z,
                aggregate,
            );
        }
    }

    let small = min_on_screen(LINE_TEXT, z, LINE_MIN);
    if call && legible {
        let button = camera.rect(card.open_button());
        Icon::ExternalLink.paint(&clipped, button.shrink(5.0 * z), theme::MUTED);
    }
    for row in &compact.rows {
        let area = cell(14.0, row.top, WIDTH - 14.0, row.top + row.height);
        let text = if full { &row.full_text } else { &row.text };
        if row.role == CompactRole::Branches {
            // *Yes* and *No* over their sockets on the bottom edge.
            for (index, (port, text)) in card.controls.iter().enumerate() {
                if !legible {
                    continue;
                }
                let x = card.control_x(index);
                let text = text.trim_end_matches(" →");
                drawn.label(
                    &clipped,
                    cell(x - 60.0, row.top, x + 60.0, row.top + row.height),
                    text,
                    TextKind::Line,
                    FontId::proportional(small),
                    match *port {
                        apiforge_core::graph::TRUE_PORT => theme::BRANCH_YES,
                        apiforge_core::graph::FALSE_PORT => theme::BRANCH_NO,
                        _ => theme::MUTED,
                    },
                    egui::Align::Center,
                );
                #[cfg(test)]
                drawn.lines.push(text.to_owned());
            }
            continue;
        }
        let (text, font, color, align) = match row.role {
            CompactRole::Caption => (
                Cow::Borrowed(text.as_str()),
                FontId::proportional(small),
                theme::MUTED,
                egui::Align::Min,
            ),
            CompactRole::Line => (
                Cow::Borrowed(text.as_str()),
                FontId::proportional(small),
                if row.missing {
                    theme::DANGER
                } else {
                    theme::TEXT
                },
                egui::Align::Min,
            ),
            CompactRole::Branches => continue,
            CompactRole::Subtitle => (
                Cow::Borrowed(text.as_str()),
                FontId::monospace(small),
                theme::LINK,
                egui::Align::Min,
            ),
            CompactRole::Input(index) => (
                Cow::Borrowed(text.as_str()),
                FontId::proportional(small),
                match card.inputs[index].value {
                    // A slot of the input reads a missing step (UX-39).
                    _ if row.missing => theme::DANGER,
                    RowValue::Missing | RowValue::MissingStep(_) => theme::DANGER,
                    RowValue::Source(_) => theme::LINK,
                    _ => theme::TEXT,
                },
                egui::Align::Min,
            ),
            CompactRole::Output(_) => (
                Cow::Borrowed(text.as_str()),
                FontId::proportional(small),
                theme::TEXT,
                egui::Align::Max,
            ),
            CompactRole::Result => {
                let top = local(0.0, row.top).y;
                clipped.line_segment(
                    [pos2(rect.left(), top), pos2(rect.right(), top)],
                    Stroke::new(0.75, theme::BORDER),
                );
                let (text, color) = step.map_or(("Not run".to_owned(), theme::MUTED), execution);
                (
                    Cow::Owned(text),
                    FontId::proportional(small),
                    color,
                    egui::Align::Min,
                )
            }
            CompactRole::Checks(_) => {
                // The checks' result, a second statement under the step's
                // own; red when one failed, at every zoom (LAY-11, Q15 = A).
                if failed_check {
                    clipped.rect_filled(
                        cell(0.0, row.top, WIDTH, row.top + row.height),
                        0.0,
                        theme::DANGER.gamma_multiply(0.12),
                    );
                }
                (
                    aggregate
                        .map(Aggregate::text)
                        .filter(|text| !text.is_empty())
                        .map_or(Cow::Borrowed(text.as_str()), Cow::Owned),
                    FontId::proportional(small),
                    if failed_check {
                        theme::DANGER
                    } else {
                        theme::MUTED
                    },
                    egui::Align::Min,
                )
            }
        };
        if legible {
            let kind = match row.role {
                CompactRole::Result | CompactRole::Checks(_) => TextKind::Result,
                CompactRole::Subtitle => TextKind::Subtitle,
                _ => TextKind::Line,
            };
            drawn.label(&clipped, area, &text, kind, font, color, align);
            #[cfg(test)]
            drawn.lines.push(text.into_owned());
        }
    }
    // Over the band and every line, at every zoom (Q15 = A).
    drawn.signal = signal(step, aggregate);
    if let Some(color) = drawn.signal {
        signal_border(painter, rect, radius, selected, color);
    }

    for index in 0..card.inputs.len() {
        if let Some(anchor) = card.shown_row_anchor(index) {
            socket(painter, camera.to_screen(anchor), false, z);
        }
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
    drawn
}

/// The trays beside a compact card (decision 2 = D): its hidden inputs on the
/// left, its hidden outputs on the right, each labelled with its socket.
pub fn trays(painter: &Painter, card: &Card, camera: Camera, inputs: bool, outputs: bool) {
    let z = camera.zoom;
    let small = min_on_screen(11.0, z, LINE_MIN);
    // A tray row is as tall as a card's, so its labels share the card's gate.
    let legible = CardDetail::at_zoom(z) != CardDetail::Overview;
    let frame = |rect: Rect| {
        let rect = camera.rect(rect);
        painter.rect_filled(
            rect.translate(vec2(0.0, 2.0 * z)),
            4.0 * z,
            Color32::from_black_alpha(10),
        );
        painter.rect_filled(rect, 4.0 * z, theme::WHITE);
        painter.rect_stroke(
            rect,
            4.0 * z,
            Stroke::new(1.0, theme::ACCENT),
            StrokeKind::Inside,
        );
    };
    if inputs && let Some(tray) = card.input_tray() {
        frame(tray);
        for index in card.hidden_inputs() {
            if let Some((row, anchor)) = card.tray_input(index) {
                if legible {
                    label(
                        painter,
                        camera.rect(row).shrink2(vec2(10.0 * z, 0.0)),
                        &card.inputs[index].label,
                        FontId::proportional(small),
                        theme::TEXT,
                        egui::Align::Min,
                    );
                }
                socket(painter, camera.to_screen(anchor), false, z);
            }
        }
    }
    if outputs && let Some(tray) = card.output_tray() {
        frame(tray);
        for index in card.hidden_outputs() {
            if let Some((row, anchor)) = card.tray_output(index) {
                if legible {
                    label(
                        painter,
                        camera.rect(row).shrink2(vec2(10.0 * z, 0.0)),
                        &card.outputs[index].label,
                        FontId::proportional(small),
                        theme::TEXT,
                        egui::Align::Max,
                    );
                }
                socket(painter, camera.to_screen(anchor), false, z);
            }
        }
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

/// Paints the header and footer bands and the steps summary, and returns where
/// the summary went. Beside the toolbar (`trailing`, KTLO-12) the summary is
/// right-aligned in the banner and left out when the banner is too narrow;
/// otherwise it starts the row.
pub fn overlay_chrome(
    painter: &Painter,
    overlays: &super::layout::BoardOverlays,
    summary: &str,
    trailing: bool,
) -> Option<Rect> {
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

    let summary_at = if trailing {
        let galley = painter.layout_no_wrap(summary.to_owned(), summary_font(), theme::MUTED);
        let banner = overlays.banner;
        (galley.size().x <= banner.width() + 0.5).then(|| {
            let at = Rect::from_min_size(
                pos2(
                    banner.right() - galley.size().x,
                    banner.center().y - galley.size().y / 2.0,
                ),
                galley.size(),
            );
            painter.galley(at.min, galley, theme::MUTED);
            at
        })
    } else {
        label(
            painter,
            overlays.banner,
            summary,
            summary_font(),
            theme::MUTED,
            egui::Align::Min,
        );
        Some(overlays.banner)
    };

    if let super::layout::HintPlacement::Inline(rect) = overlays.hint {
        painter.text(
            rect.left_center(),
            Align2::LEFT_CENTER,
            SHORTCUT_HINT_TEXT,
            FontId::proportional(theme::FONT_META),
            theme::MUTED,
        );
    }
    summary_at
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
                let mut card = Card::build(
                    node,
                    graph,
                    &project,
                    super::super::layout::CardDensity::Full,
                );
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
                    |ui| {
                        super::card(ui.painter(), &card, camera, true, false, None, None);
                    },
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

    /// The compact card and its trays (LAY-16) keep every label inside its cell
    /// at every zoom, and below the legible row height show only the title.
    #[test]
    fn every_compact_card_label_stays_in_its_cell_from_25_to_160_percent() {
        let context = egui::Context::default();
        theme::apply(&context);
        let project = apiforge_core::demos::shop_project();
        let mut compact = 0;
        let mut trays = 0;
        let graphs: Vec<_> = project
            .scenarios
            .iter()
            .flat_map(|scenario| [&scenario.graph, &scenario.cleanup])
            .collect();
        for zoom in [0.25, 0.35, 0.45, 0.6, 0.75, 1.0, 1.6] {
            for (graph, node) in graphs
                .iter()
                .flat_map(|graph| graph.nodes.iter().map(move |node| (*graph, node)))
            {
                let mut card = Card::build(
                    node,
                    graph,
                    &project,
                    super::super::layout::CardDensity::Compact,
                );
                if card.compact.is_none() {
                    continue;
                }
                compact += 1;
                card.origin = pos2(200.0, 0.0);
                card.name = "ŻółćVeryLongUnbrokenComponentName".repeat(5);
                for row in &mut card.inputs {
                    row.label = "Extremely long argument label".repeat(4);
                }
                for row in &mut card.outputs {
                    row.label = "long_output_name_without_spaces".repeat(4);
                }
                let camera = Camera {
                    zoom,
                    pan: vec2(96.0, 96.0),
                };
                let mut cells = vec![camera.rect(card.rect())];
                cells.extend(card.input_tray().map(|tray| camera.rect(tray)));
                cells.extend(card.output_tray().map(|tray| camera.rect(tray)));
                trays += cells.len() - 1;
                let mut output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(2400.0, 1600.0))),
                        ..Default::default()
                    },
                    |ui| {
                        super::compact_card(ui.painter(), &card, camera, true, false, None, None);
                        super::trays(ui.painter(), &card, camera, true, true);
                    },
                );
                // A value's icon is a glyph, not a label (LAY-21).
                let icon = camera.rect(Rect::from_min_size(
                    card.origin + vec2(12.0, 10.0),
                    Vec2::splat(22.0),
                ));
                let mut labels = 0;
                for shape in &output.shapes {
                    if let Shape::Text(text) = &shape.shape {
                        let text_bounds = text.galley.rect.translate(text.pos.to_vec2());
                        if !icon.contains(text_bounds.center()) {
                            labels += 1;
                        }
                        assert!(
                            cells
                                .iter()
                                .any(|cell| cell.expand(0.75).contains_rect(shape.clip_rect)),
                            "label clip escapes card and trays at {zoom}: {:?}",
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
                    assert_eq!(labels, 1, "only the title is drawn at {zoom}");
                }
                output.textures_delta.clear();
            }
        }
        assert!(compact > 0, "the board has request cards");
        assert!(trays > 0, "the board has a card with a tray");
    }
}

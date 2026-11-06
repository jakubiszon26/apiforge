//! LAY-16 mock-up (throwaway branch `mock/lay-16-inputs`): the compact request
//! card, and four ways to reach the ports it does not show. Nothing here is
//! meant to be merged; the review views in `folder::lay16_review` render it.
use super::{
    Camera,
    layout::{COMPACT_HEADER, COMPACT_ROW, Card, Compact, CompactRole, RowValue, WIDTH},
    paint::{socket, status_color},
};
use crate::mock_lay16::{self as mock, Option_};
use crate::{theme, widgets::paint_label as label};
use apiforge_core::{
    format::{Graph, Project},
    graph::{URL_INPUT, wires},
    model::NodeKind,
    run::StepResult,
};
use eframe::egui::{
    self, Color32, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2, pos2, vec2,
};
use std::collections::HashSet;

/// Option D's tray: its width and its gap from the card.
pub const TRAY_WIDTH: f32 = 88.0;
pub const TRAY_GAP: f32 = 4.0;
/// Option A's edge sockets: their pitch along the edge.
const EDGE_PITCH: f32 = 16.0;
/// Option D: a tray appears while a wire is dragged within this distance.
const NEAR: f32 = 120.0;

/// Gives request and component cards the compact box of the option.
pub fn apply(cards: &mut [Card], graph: &Graph, _project: &Project, option: Option_) {
    if option == Option_::Today {
        return;
    }
    let wires = wires(graph);
    for card in cards.iter_mut() {
        if !matches!(card.kind, NodeKind::Request | NodeKind::Component) {
            continue;
        }
        let Some(node) = graph.nodes.iter().find(|node| node.id == card.id) else {
            continue;
        };
        let used_inputs: HashSet<&str> = wires
            .iter()
            .filter(|wire| wire.target == card.id)
            .filter_map(|wire| wire.target_input.as_deref())
            .collect();
        let used_outputs: HashSet<&str> = wires
            .iter()
            .filter(|wire| wire.source == card.id)
            .filter_map(|wire| wire.source_port.as_deref())
            .collect();
        let checks = graph
            .nodes
            .iter()
            .filter(|other| other.kind.is_check())
            .filter(|other| {
                wires
                    .iter()
                    .any(|wire| wire.source == card.id && wire.target == other.id)
            })
            .count();
        let mut roles = Vec::new();
        if card.subtitle.is_some() {
            roles.push(CompactRole::Subtitle);
        }
        for (index, row) in card.inputs.iter().enumerate() {
            if row.name == URL_INPUT {
                continue;
            }
            let configured = row.path.is_empty() && node.inputs.contains_key(&row.name);
            let missing = row.required && row.value == RowValue::Missing;
            if configured
                || missing
                || used_inputs.contains(row.name.as_str())
                || option == Option_::C
            {
                roles.push(CompactRole::Input(index));
            }
        }
        for (index, row) in card.outputs.iter().enumerate() {
            if used_outputs.contains(row.name.as_str()) || (option == Option_::C && index == 0) {
                roles.push(CompactRole::Output(index));
            }
        }
        roles.push(CompactRole::Result);
        if checks > 0 {
            roles.push(CompactRole::Checks(checks));
        }
        card.compact = Some(Compact::new(&roles));
    }
}

/// Moves the cards the view places by hand.
pub fn place(cards: &mut [Card]) {
    for (id, origin) in mock::get().place {
        if let Some(card) = cards.iter_mut().find(|card| card.id == id) {
            card.origin = origin;
        }
    }
}

/// The inputs a compact card hides (the URL has the subtitle's socket).
pub fn hidden_inputs(card: &Card) -> Vec<usize> {
    (0..card.inputs.len())
        .filter(|index| card.inputs[*index].name != URL_INPUT && !card.compact_shows_input(*index))
        .collect()
}

pub fn hidden_outputs(card: &Card) -> Vec<usize> {
    (0..card.outputs.len())
        .filter(|index| !card.compact_shows_output(*index))
        .collect()
}

/// Option A: the k-th of n hidden sockets along an edge, from `start` down,
/// 16 px apart, squeezed to stay inside the box.
fn edge_y(card: &Card, start: f32, k: usize, n: usize) -> f32 {
    let end = card.height() - 8.0;
    let pitch = if n <= 1 {
        EDGE_PITCH
    } else {
        EDGE_PITCH.min((end - start) / (n - 1) as f32)
    };
    start + k as f32 * pitch
}

fn subtitle_bottom(card: &Card) -> f32 {
    card.compact
        .as_ref()
        .and_then(|compact| compact.row(CompactRole::Subtitle))
        .map_or(COMPACT_HEADER, |row| row.top + row.height)
}

/// Option D: the tray of hidden inputs, left of the card.
pub fn input_tray(card: &Card) -> Rect {
    let n = hidden_inputs(card).len().max(1) as f32;
    Rect::from_min_size(
        card.origin + vec2(-TRAY_GAP - TRAY_WIDTH, COMPACT_HEADER),
        vec2(TRAY_WIDTH, n * COMPACT_ROW + 8.0),
    )
}

/// Option D: the tray of hidden outputs, right of the card.
pub fn output_tray(card: &Card) -> Rect {
    let n = hidden_outputs(card).len().max(1) as f32;
    Rect::from_min_size(
        card.origin + vec2(WIDTH + TRAY_GAP, COMPACT_HEADER),
        vec2(TRAY_WIDTH, n * COMPACT_ROW + 8.0),
    )
}

fn tray_row(tray: Rect, k: usize) -> Rect {
    Rect::from_min_size(
        tray.min + vec2(0.0, 4.0 + k as f32 * COMPACT_ROW),
        vec2(TRAY_WIDTH, COMPACT_ROW),
    )
}

/// Where a hidden input's socket is under the option: A on the card's left
/// edge, D at the tray's left edge; B and C have none.
pub fn hidden_input_anchor(card: &Card, index: usize) -> Option<Pos2> {
    let hidden = hidden_inputs(card);
    let k = hidden.iter().position(|i| *i == index)?;
    match mock::option() {
        Option_::A => Some(
            card.origin
                + vec2(
                    0.0,
                    edge_y(card, subtitle_bottom(card) + 10.0, k, hidden.len()),
                ),
        ),
        Option_::D => {
            let row = tray_row(input_tray(card), k);
            Some(pos2(row.left(), row.center().y))
        }
        _ => None,
    }
}

pub fn hidden_output_anchor(card: &Card, index: usize) -> Option<Pos2> {
    let hidden = hidden_outputs(card);
    let k = hidden.iter().position(|i| *i == index)?;
    match mock::option() {
        Option_::A => {
            Some(card.origin + vec2(WIDTH, edge_y(card, COMPACT_HEADER + 8.0, k, hidden.len())))
        }
        Option_::D => {
            let row = tray_row(output_tray(card), k);
            Some(pos2(row.right(), row.center().y))
        }
        _ => None,
    }
}

/// The drop rect of a hidden input for a dragged variable.
pub fn hidden_input_rect(card: &Card, index: usize) -> Rect {
    match mock::option() {
        Option_::A => hidden_input_anchor(card, index).map_or(Rect::NOTHING, |anchor| {
            Rect::from_center_size(anchor, vec2(28.0, 16.0))
        }),
        Option_::D => {
            let hidden = hidden_inputs(card);
            hidden
                .iter()
                .position(|i| *i == index)
                .map_or(Rect::NOTHING, |k| tray_row(input_tray(card), k))
        }
        _ => Rect::NOTHING,
    }
}

/// What a compact card reveals of its hidden ports this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reveal {
    pub inputs: bool,
    pub outputs: bool,
}

pub(super) fn reveal(
    card: &Card,
    camera: Camera,
    selected: bool,
    dragging: bool,
    pointer: Option<Pos2>,
) -> Reveal {
    if card.compact.is_none() {
        return Reveal::default();
    }
    match mock::option() {
        Option_::A => Reveal {
            inputs: dragging || selected,
            outputs: selected,
        },
        Option_::D => {
            let near = pointer.is_some_and(|pointer| {
                camera
                    .rect(card.rect())
                    .expand(NEAR * camera.zoom)
                    .contains(pointer)
            });
            Reveal {
                inputs: selected || (dragging && near),
                outputs: selected,
            }
        }
        _ => Reveal::default(),
    }
}

fn input_line(card: &Card, index: usize) -> (String, Color32) {
    let row = &card.inputs[index];
    match &row.value {
        RowValue::Inline { text, .. } | RowValue::TypeChoice(text) => {
            (format!("{} · {text}", row.label), theme::TEXT)
        }
        RowValue::Source(text) => (format!("{} ← {text}", row.label), theme::LINK),
        RowValue::Summary(text) => (format!("{} · {text}", row.label), theme::TEXT),
        RowValue::Missing => (format!("{} · value missing", row.label), theme::DANGER),
        RowValue::Optional => (format!("{} · optional", row.label), theme::MUTED),
    }
}

/// The compact card: band, name, rows, result line, and the sockets of the
/// rows it shows.
pub(super) fn paint_card(
    painter: &Painter,
    card: &Card,
    camera: Camera,
    selected: bool,
    related: bool,
    step: Option<&StepResult>,
) {
    let Some(compact) = &card.compact else {
        return;
    };
    let z = camera.zoom;
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

    let band_h = (5.0 * z).max(3.0);
    let band = Rect::from_min_max(rect.left_top(), pos2(rect.right(), rect.top() + band_h));
    let r = radius as u8;
    let corners = egui::CornerRadius {
        nw: r,
        ne: r,
        sw: 0,
        se: 0,
    };
    clipped.rect_filled(
        band,
        corners,
        step.map_or(Color32::from_rgb(214, 214, 214), |step| {
            status_color(step.status)
        }),
    );

    let color = theme::block_color(card.block);
    let icon = Rect::from_min_size(local(12.0, 10.0), Vec2::splat(22.0 * z));
    clipped.rect_filled(icon, 5.0 * z, color.gamma_multiply(0.10));
    theme::block_icon(&clipped, icon, card.block, color);
    label(
        &clipped,
        cell(42.0, 6.0, WIDTH - 12.0, COMPACT_HEADER),
        &card.name,
        FontId::new((16.0 * z).max(11.0), theme::bold_family()),
        theme::TEXT,
        egui::Align::Min,
    );
    let small = (12.0 * z).max(9.0);
    let rows_text = COMPACT_ROW * z >= 11.0;
    for row in &compact.rows {
        let area = cell(14.0, row.top, WIDTH - 14.0, row.top + row.height);
        match row.role {
            CompactRole::Subtitle if rows_text => label(
                &clipped,
                area,
                card.subtitle.as_deref().unwrap_or_default(),
                FontId::monospace(small),
                theme::LINK,
                egui::Align::Min,
            ),
            CompactRole::Input(index) if rows_text => {
                let (text, color) = input_line(card, index);
                label(
                    &clipped,
                    area,
                    &text,
                    FontId::proportional(small),
                    color,
                    egui::Align::Min,
                );
            }
            CompactRole::Output(index) if rows_text => label(
                &clipped,
                area,
                &format!("{} →", card.outputs[index].label),
                FontId::proportional(small),
                theme::TEXT,
                egui::Align::Max,
            ),
            CompactRole::Result => {
                let top = local(0.0, row.top);
                clipped.line_segment(
                    [pos2(rect.left(), top.y), pos2(rect.right(), top.y)],
                    Stroke::new(0.75, theme::BORDER),
                );
                let (text, color) = step.map_or(("Not run".to_owned(), theme::MUTED), |step| {
                    (
                        format!("{} · {} ms", step.status.label(), step.duration_ms),
                        status_color(step.status),
                    )
                });
                if rows_text {
                    label(
                        &clipped,
                        area,
                        &text,
                        FontId::proportional(small),
                        color,
                        egui::Align::Min,
                    );
                }
            }
            CompactRole::Checks(count) if rows_text => label(
                &clipped,
                area,
                &format!("{count} {}", if count == 1 { "check" } else { "checks" }),
                FontId::proportional(small),
                theme::MUTED,
                egui::Align::Min,
            ),
            _ => {}
        }
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
}

/// Option A's edge sockets with their labels inside the box, or option D's
/// trays beside it, for what the card reveals this frame.
pub(super) fn paint_affordances(painter: &Painter, card: &Card, camera: Camera, reveal: Reveal) {
    if card.compact.is_none() || reveal == Reveal::default() {
        return;
    }
    let z = camera.zoom;
    let small = (11.0 * z).max(8.0);
    match mock::option() {
        Option_::A => {
            if reveal.inputs {
                for index in hidden_inputs(card) {
                    let Some(anchor) = hidden_input_anchor(card, index) else {
                        continue;
                    };
                    let at = camera.to_screen(anchor);
                    let text = card.inputs[index].label.clone();
                    let width = text.chars().count() as f32 * 6.4 * z + 10.0 * z;
                    let pill =
                        Rect::from_min_size(at + vec2(8.0 * z, -7.0 * z), vec2(width, 14.0 * z));
                    painter.rect_filled(pill, 3.0 * z, Color32::from_white_alpha(230));
                    label(
                        painter,
                        pill.shrink2(vec2(4.0 * z, 0.0)),
                        &text,
                        FontId::proportional(small),
                        theme::MUTED,
                        egui::Align::Min,
                    );
                    socket(painter, at, false, z);
                }
            }
            if reveal.outputs {
                for index in hidden_outputs(card) {
                    let Some(anchor) = hidden_output_anchor(card, index) else {
                        continue;
                    };
                    let at = camera.to_screen(anchor);
                    let text = card.outputs[index].label.clone();
                    let width = text.chars().count() as f32 * 6.4 * z + 10.0 * z;
                    let pill = Rect::from_min_size(
                        at + vec2(-8.0 * z - width, -7.0 * z),
                        vec2(width, 14.0 * z),
                    );
                    painter.rect_filled(pill, 3.0 * z, Color32::from_white_alpha(230));
                    label(
                        painter,
                        pill.shrink2(vec2(4.0 * z, 0.0)),
                        &text,
                        FontId::proportional(small),
                        theme::MUTED,
                        egui::Align::Max,
                    );
                    socket(painter, at, false, z);
                }
            }
        }
        Option_::D => {
            let tray = |painter: &Painter, rect: Rect| {
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
            if reveal.inputs && !hidden_inputs(card).is_empty() {
                tray(painter, input_tray(card));
                for (k, index) in hidden_inputs(card).into_iter().enumerate() {
                    let row = camera.rect(tray_row(input_tray(card), k));
                    label(
                        painter,
                        row.shrink2(vec2(10.0 * z, 0.0)),
                        &card.inputs[index].label,
                        FontId::proportional(small),
                        theme::TEXT,
                        egui::Align::Min,
                    );
                    if let Some(anchor) = hidden_input_anchor(card, index) {
                        socket(painter, camera.to_screen(anchor), false, z);
                    }
                }
            }
            if reveal.outputs && !hidden_outputs(card).is_empty() {
                tray(painter, output_tray(card));
                for (k, index) in hidden_outputs(card).into_iter().enumerate() {
                    let row = camera.rect(tray_row(output_tray(card), k));
                    label(
                        painter,
                        row.shrink2(vec2(10.0 * z, 0.0)),
                        &card.outputs[index].label,
                        FontId::proportional(small),
                        theme::TEXT,
                        egui::Align::Max,
                    );
                    if let Some(anchor) = hidden_output_anchor(card, index) {
                        socket(painter, camera.to_screen(anchor), false, z);
                    }
                }
            }
        }
        _ => {}
    }
}

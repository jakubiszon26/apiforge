//! LAY-24 mock-up (throwaway, never merged): what stands for an `@after`
//! rail into the next step of the spine when the rail itself is not drawn.
//! Placeholder drawing over the real board; nothing here edits or saves.

use super::Camera;
use super::layout::{COMPACT_HEADER, Card, WIDTH};
use super::routing::{BoardWire, Start};
use super::shape;
use crate::theme;
use apiforge_core::{format::Graph, model::NodeKind};
use eframe::egui::{Color32, FontId, Painter, Pos2, Rect, Stroke, pos2, vec2};
use std::collections::HashMap;

/// The option rendered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// Today: every rail is drawn.
    #[default]
    Off,
    /// A: `after quantity_ok` on a tab standing on the card's top edge.
    Tab,
    /// A, the other placement: the same words inside the card's header.
    Header,
    /// B: a dashed arrow into the step's badge, no words.
    Glyph,
    /// C: nothing at rest; the rail is drawn for a selected card.
    OnDemand,
}

/// A rail into the next step that the board does not draw.
#[derive(Clone, Debug)]
pub struct Mark {
    pub wire: String,
    /// The card the rail enters, as an index into the cards.
    pub target: usize,
    /// The card it was drawn from: the step, or a folded check's subject.
    pub source: String,
    /// `after quantity_ok`.
    pub text: String,
}

/// The ordering wires that enter the next step of the spine: the target is
/// on the spine and the wire comes from the column just before it, from
/// the step there or from a check hung under it.
pub fn marks(
    graph: &Graph,
    cards: &[Card],
    wires: &[BoardWire],
    labels: &HashMap<String, String>,
) -> Vec<Mark> {
    let Some(plan) = shape::plan(graph, None) else {
        return Vec::new();
    };
    let column = |id: &str| -> Option<usize> {
        let index = graph.nodes.iter().position(|node| node.id == id)?;
        plan.spine.iter().position(|at| *at == index).or_else(|| {
            plan.satellites
                .iter()
                .find(|(at, _)| *at == index)
                .map(|(_, under)| *under)
        })
    };
    wires
        .iter()
        .filter(|wire| wire.control && wire.branch.is_none())
        .filter_map(|wire| {
            let Start::Output { card, .. } = &wire.start else {
                return None;
            };
            let (source, target) = (&cards[*card], &cards[wire.target]);
            if !shape::on_spine(target.kind) {
                return None;
            }
            (column(&target.id)? == column(&source.id)? + 1).then(|| Mark {
                wire: wire.id.clone(),
                target: wire.target,
                source: source.id.clone(),
                text: labels
                    .get(&wire.id)
                    .cloned()
                    .unwrap_or_else(|| format!("after {}", source.name)),
            })
        })
        .collect()
}

fn dashed(painter: &Painter, from: Pos2, to: Pos2, stroke: Stroke) {
    let length = from.distance(to);
    if length < 1.0 {
        return;
    }
    let direction = (to - from) / length;
    let (on, off) = (3.5, 2.5);
    let mut at = 0.0;
    while at < length {
        painter.line_segment(
            [
                from + direction * at,
                from + direction * (at + on).min(length),
            ],
            stroke,
        );
        at += on + off;
    }
}

/// Draws the mark of `mode` on its card; `lit` while its rail is drawn.
pub fn paint(painter: &Painter, mark: &Mark, card: &Card, camera: Camera, mode: Mode, lit: bool) {
    let z = camera.zoom;
    let rect = camera.rect(card.rect());
    let color = if lit { theme::ACCENT } else { theme::MUTED };
    let font = FontId::proportional((11.0 * z).max(9.0));
    let stroke = Stroke::new(1.3, color);
    // The badge is a circle on the card's top-left corner (LAY-07).
    let badge = (10.0 * z).max(7.5);
    match mode {
        Mode::Off | Mode::OnDemand => {}
        Mode::Header => {
            // Right-aligned in the header, after the name; shortened to
            // the dashes alone where the name leaves no room.
            let name = painter
                .layout_no_wrap(
                    card.name.clone(),
                    FontId::new((16.0 * z).max(11.0), theme::bold_family()),
                    theme::TEXT,
                )
                .size()
                .x;
            let right = if card.kind == NodeKind::Component {
                WIDTH - 40.0
            } else {
                WIDTH - 12.0
            };
            let left = camera.to_screen(card.origin + vec2(42.0, 0.0)).x + name + 8.0;
            let right = camera.to_screen(card.origin + vec2(right, 0.0)).x;
            let middle = camera
                .to_screen(card.origin + vec2(0.0, (6.0 + COMPACT_HEADER) / 2.0))
                .y
                .max(rect.top() + font.size / 2.0 + 4.0);
            let lead = 12.0;
            let galley = painter.layout_no_wrap(mark.text.clone(), font, color);
            let whole = galley.size().x + lead + 4.0;
            if right - left >= whole {
                let start = right - whole;
                dashed(
                    painter,
                    pos2(start, middle),
                    pos2(start + lead, middle),
                    stroke,
                );
                painter.galley(
                    pos2(start + lead + 4.0, middle - galley.size().y / 2.0),
                    galley,
                    color,
                );
            } else if right - left >= lead {
                dashed(
                    painter,
                    pos2(right - lead, middle),
                    pos2(right, middle),
                    stroke,
                );
            }
        }
        Mode::Tab => {
            // A tab standing on the top edge, right of the badge.
            let galley = painter.layout_no_wrap(mark.text.clone(), font, color);
            let lead = 12.0;
            // Where the card is narrower than the words, the dashes alone.
            let room = rect.width() - badge - 4.0;
            let words = galley.size().x + lead + 12.0 <= room;
            let size = vec2(
                if words {
                    galley.size().x + lead + 12.0
                } else {
                    lead + 8.0
                },
                galley.size().y + 3.0,
            );
            let tab =
                Rect::from_min_size(pos2(rect.left() + badge + 4.0, rect.top() - size.y), size);
            painter.rect_filled(tab, 3.0, Color32::from_rgb(244, 244, 246));
            painter.rect_stroke(
                tab,
                3.0,
                Stroke::new(1.0, if lit { theme::ACCENT } else { theme::BORDER }),
                eframe::egui::StrokeKind::Inside,
            );
            let middle = tab.center().y;
            dashed(
                painter,
                pos2(tab.left() + 4.0, middle),
                pos2(tab.left() + 4.0 + lead, middle),
                stroke,
            );
            if words {
                painter.galley(
                    pos2(tab.left() + lead + 8.0, tab.top() + 1.5),
                    galley,
                    color,
                );
            }
        }
        Mode::Glyph => {
            // A dashed arrow into the badge, level with the corner.
            let tip = pos2(rect.left() - badge - 2.0, rect.top());
            let tail = tip - vec2((22.0 * z).max(14.0), 0.0);
            dashed(painter, tail, tip - vec2(3.0, 0.0), Stroke::new(1.6, color));
            let head = 4.5;
            painter.add(eframe::egui::Shape::convex_polygon(
                vec![
                    tip,
                    tip + vec2(-head * 1.4, -head),
                    tip + vec2(-head * 1.4, head),
                ],
                color,
                Stroke::NONE,
            ));
        }
    }
}

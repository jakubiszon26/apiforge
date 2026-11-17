//! Mock of LAY-13 (#297): branch bands. Throwaway. `APIFORGE_LAY13=a` draws
//! a band round the branch's steps with everything hung under them, `=b`
//! round the statements of the braces alone; unset is `main`.
use super::Camera;
use super::paint;
use super::shape::on_spine;
use crate::theme;
use apiforge_core::{
    format::Graph,
    graph::sequence,
    model::{Branch, NodeKind},
    run::{StepResult, StepStatus},
};
use eframe::egui::{
    Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, pos2, vec2,
};
use std::collections::HashMap;

pub fn variant() -> Option<char> {
    std::env::var("APIFORGE_LAY13").ok()?.chars().next()
}

/// The space between a band's edge and its cards, and the header above them.
const PAD: f32 = 10.0;
/// The header's strip; with the 4 px under it, 20 px above the cards, which
/// the 24 px above a hung card (LAY-31) holds.
const HEADER: f32 = 16.0;
const UNDER: f32 = 4.0;

/// One row of a branch: its columns left to right, each with its own
/// bottom, under one top.
#[derive(Clone, Debug)]
pub struct Fragment {
    pub top: f32,
    /// Left, right and bottom of each column, on the board.
    pub columns: Vec<(f32, f32, f32)>,
    pub cards: Vec<String>,
    /// Where the header's words start: right of the first card's badge.
    pub words: f32,
}

impl Fragment {
    pub fn bounds(&self) -> Rect {
        let left = self.columns.first().map_or(0.0, |column| column.0);
        let right = self.columns.last().map_or(0.0, |column| column.1);
        let bottom = self.columns.iter().map(|column| column.2).fold(self.top, f32::max);
        Rect::from_min_max(pos2(left, self.top), pos2(right, bottom))
    }
}

#[derive(Clone, Debug)]
pub struct Band {
    pub source: String,
    pub handle: String,
    pub branch: Branch,
    /// The statements of the braces, in the engine's order.
    pub members: Vec<String>,
    pub fragments: Vec<Fragment>,
}

/// The braces every statement stands in, as `apiforge-script` reads them:
/// it follows `x.b` and everything else it follows stands in the same
/// braces.
pub fn braces(graph: &Graph) -> HashMap<String, (String, Branch)> {
    let is_if = |id: &str| {
        graph
            .nodes
            .iter()
            .any(|node| node.id == id && node.kind == NodeKind::If)
    };
    let mut incoming: HashMap<String, Vec<(String, Option<Branch>)>> = HashMap::new();
    for edge in &graph.edges {
        incoming
            .entry(edge.target.clone())
            .or_default()
            .push((edge.source.clone(), edge.branch));
    }
    let mut parents: HashMap<String, (String, Branch)> = HashMap::new();
    for id in sequence(graph) {
        let Some(edges) = incoming.get(id) else {
            continue;
        };
        let within = |start: &String, braces: &(String, Branch)| {
            let mut current = start.clone();
            for _ in 0..=graph.nodes.len() {
                match parents.get(&current) {
                    Some(parent) if parent == braces => return true,
                    Some((source, _)) => current = source.clone(),
                    None => return false,
                }
            }
            false
        };
        let parent = edges
            .iter()
            .filter_map(|(source, branch)| Some((source.clone(), (*branch)?)))
            .filter(|(source, _)| is_if(source))
            .find(|braces| {
                edges.iter().all(|(source, other)| {
                    (source == &braces.0 && *other == Some(braces.1)) || within(source, braces)
                })
            });
        if let Some(parent) = parent {
            parents.insert(id.to_owned(), parent);
        }
    }
    parents
}

/// The bands of a board whose cards lie at `cards` (id, kind, place).
pub fn bands(graph: &Graph, cards: &[(String, NodeKind, Rect)], whole_groups: bool) -> Vec<Band> {
    let parents = braces(graph);
    let order = sequence(graph);
    let mut rows: Vec<f32> = cards
        .iter()
        .filter(|(_, kind, _)| on_spine(*kind))
        .map(|(_, _, rect)| rect.top())
        .collect();
    rows.sort_by(f32::total_cmp);
    rows.dedup_by(|a, b| (*a - *b).abs() < 1.0);
    let row_of = |rect: &Rect| rows.iter().rposition(|top| *top <= rect.top() + 1.0).unwrap_or(0);
    let mut result: Vec<Band> = Vec::new();
    for id in &order {
        let Some((source, branch)) = parents.get(*id).cloned() else {
            continue;
        };
        match result
            .iter_mut()
            .find(|band| band.source == source && band.branch == branch)
        {
            Some(band) => band.members.push((*id).to_owned()),
            None => result.push(Band {
                handle: graph
                    .nodes
                    .iter()
                    .find(|node| node.id == source)
                    .map_or_else(String::new, |node| node.handle.clone()),
                source,
                branch,
                members: vec![(*id).to_owned()],
                fragments: Vec::new(),
            }),
        }
    }
    for band in &mut result {
        let mut held: Vec<&(String, NodeKind, Rect)> = cards
            .iter()
            .filter(|(id, ..)| band.members.contains(id))
            .collect();
        if whole_groups {
            // Everything hung under a step of the branch goes with it.
            let spine: Vec<Rect> = held
                .iter()
                .filter(|(_, kind, _)| on_spine(*kind))
                .map(|(_, _, rect)| *rect)
                .collect();
            for card in cards {
                let hung = !on_spine(card.1)
                    && spine.iter().any(|step| {
                        (step.left() - card.2.left()).abs() < 1.0
                            && card.2.top() >= step.bottom()
                            && row_of(step) == row_of(&card.2)
                    });
                if hung && !held.iter().any(|other| other.0 == card.0) {
                    held.push(card);
                }
            }
        }
        let mut by_row: Vec<(usize, Vec<&(String, NodeKind, Rect)>)> = Vec::new();
        for card in held {
            let row = row_of(&card.2);
            match by_row.iter_mut().find(|(other, _)| *other == row) {
                Some((_, cards)) => cards.push(card),
                None => by_row.push((row, vec![card])),
            }
        }
        by_row.sort_by_key(|(row, _)| *row);
        for (_, cards) in by_row {
            let mut columns: Vec<Rect> = Vec::new();
            for card in &cards {
                match columns
                    .iter_mut()
                    .find(|column| (column.left() - card.2.left()).abs() < 1.0)
                {
                    Some(column) => *column = column.union(card.2),
                    None => columns.push(card.2),
                }
            }
            columns.sort_by(|a, b| a.left().total_cmp(&b.left()));
            let top = columns.iter().map(Rect::top).fold(f32::INFINITY, f32::min) - HEADER - UNDER;
            let lowest = columns.iter().map(Rect::bottom).fold(0.0, f32::max);
            let count = columns.len();
            let shaped: Vec<(f32, f32, f32)> = (0..count)
                .map(|index| {
                    let column = columns[index];
                    let left = match index.checked_sub(1) {
                        Some(before) => (columns[before].right() + column.left()) / 2.0,
                        None => column.left() - PAD,
                    };
                    let right = match columns.get(index + 1) {
                        Some(after) => (column.right() + after.left()) / 2.0,
                        None => column.right() + PAD,
                    };
                    let bottom = if whole_groups { lowest } else { column.bottom() };
                    (left, right, bottom + PAD)
                })
                .collect();
            band.fragments.push(Fragment {
                top,
                columns: shaped,
                cards: cards.iter().map(|card| card.0.clone()).collect(),
                words: columns[0].left() + 18.0,
            });
        }
    }
    result.retain(|band| !band.fragments.is_empty());
    result
}

fn tint() -> (Color32, Color32) {
    (
        Color32::from_rgba_unmultiplied(92, 108, 132, 22),
        Color32::from_rgba_unmultiplied(92, 108, 132, 110),
    )
}

/// The band's fragments on screen, widened to hold the chips at their
/// cards' sockets.
pub fn screen(
    band: &Band,
    camera: Camera,
    chips: &[Rect],
) -> Vec<(Rect, Vec<(f32, f32, f32)>, f32)> {
    band.fragments
        .iter()
        .map(|fragment| {
            let mut columns: Vec<(f32, f32, f32)> = fragment
                .columns
                .iter()
                .map(|(left, right, bottom)| {
                    (
                        camera.to_screen(pos2(*left, 0.0)).x,
                        camera.to_screen(pos2(*right, 0.0)).x,
                        camera.to_screen(pos2(0.0, *bottom)).y,
                    )
                })
                .collect();
            let top = camera.to_screen(pos2(0.0, fragment.top)).y;
            let reach = 110.0 * camera.zoom;
            let margin = 6.0 * camera.zoom;
            let lowest = columns.iter().map(|column| column.2).fold(top, f32::max);
            if let Some(first) = columns.first_mut() {
                for chip in chips {
                    if chip.right() <= first.0 + PAD * camera.zoom + 2.0
                        && chip.right() > first.0 - reach
                        && chip.top() > top
                        && chip.bottom() < lowest
                    {
                        first.0 = first.0.min(chip.left() - margin);
                    }
                }
            }
            if let Some(last) = columns.last_mut() {
                for chip in chips {
                    if chip.left() >= last.1 - PAD * camera.zoom - 2.0
                        && chip.left() < last.1 + reach
                        && chip.top() > top
                        && chip.bottom() < lowest
                    {
                        last.1 = last.1.max(chip.right() + margin);
                    }
                }
            }
            let left = columns.first().map_or(0.0, |column| column.0);
            let right = columns.last().map_or(0.0, |column| column.1);
            (
                Rect::from_min_max(pos2(left, top), pos2(right, lowest)),
                columns,
                camera.to_screen(pos2(fragment.words, 0.0)).x,
            )
        })
        .collect()
}

/// Draws the band under the wires and cards.
pub fn paint(
    painter: &Painter,
    band: &Band,
    shapes: &[(Rect, Vec<(f32, f32, f32)>, f32)],
    camera: Camera,
    steps: Option<&[StepResult]>,
) {
    let (fill, line) = tint();
    let status = steps.and_then(|steps| {
        let of: Vec<StepStatus> = steps
            .iter()
            .filter(|step| band.members.contains(&step.node_id))
            .map(|step| step.status)
            .collect();
        if of.is_empty() {
            None
        } else if of.iter().all(|status| *status == StepStatus::Skipped) {
            Some("skipped")
        } else if of
            .iter()
            .any(|status| matches!(status, StepStatus::Failed | StepStatus::Error))
        {
            Some("failed")
        } else {
            Some("ran")
        }
    });
    for (index, (bounds, columns, words)) in shapes.iter().enumerate() {
        let top = bounds.top();
        for (left, right, bottom) in columns {
            painter.rect_filled(
                Rect::from_min_max(pos2(*left, top), pos2(*right, *bottom)),
                0.0,
                fill,
            );
        }
        let mut outline: Vec<Pos2> = vec![pos2(bounds.left(), top), pos2(bounds.right(), top)];
        for (left, right, bottom) in columns.iter().rev() {
            outline.push(pos2(*right, *bottom));
            outline.push(pos2(*left, *bottom));
        }
        outline.dedup();
        painter.add(Shape::closed_line(outline, Stroke::new(1.0, line)));
        let _ = StrokeKind::Inside;
        let mut text = format!(
            "{} → {}",
            band.handle,
            match band.branch {
                Branch::True => "Yes",
                Branch::False => "No",
            }
        );
        if index > 0 {
            text.push_str(" · continued");
        }
        if let Some(status) = status {
            text.push_str(" · ");
            text.push_str(status);
        }
        // Below 45 % the strip is lower than the smallest words: the band
        // alone is drawn.
        if (HEADER + UNDER) * camera.zoom < 9.0 {
            continue;
        }
        let font = FontId::proportional((12.0 * camera.zoom).max(9.0));
        let mut job = eframe::egui::text::LayoutJob::simple_singleline(
            text,
            font,
            Color32::from_rgb(71, 85, 105),
        );
        job.wrap = eframe::egui::text::TextWrapping::truncate_at_width(
            (bounds.right() - words - 6.0 * camera.zoom).max(8.0),
        );
        let galley = painter.layout_job(job);
        let height = ((HEADER + UNDER) * camera.zoom).max(galley.size().y);
        let at = pos2(
            *words,
            top + (height - galley.size().y) / 2.0,
        );
        painter.galley(at, galley, Color32::from_rgb(71, 85, 105));
        let _ = (Align2::LEFT_TOP, vec2(0.0, 0.0), theme::MUTED);
    }
}

/// The one wire of the branch, drawn as far as the band's edge.
pub fn entry(
    painter: &Painter,
    line: &[Pos2],
    into: Rect,
    obstacles: &[Rect],
    branch: Branch,
    zoom: f32,
    dim: bool,
) {
    let mut color = match branch {
        Branch::True => theme::BRANCH_YES,
        Branch::False => theme::BRANCH_NO,
    };
    if dim {
        color = color.gamma_multiply(0.35);
    }
    // Beside the band: straight from the port into the band's side, at
    // the header's height.
    if let Some(start) = line.first().copied() {
        let level = into.top() + (HEADER + UNDER) * zoom / 2.0;
        let (edge, clear) = if start.x < into.left() {
            (into.left(), true)
        } else if start.x > into.right() {
            (into.right(), true)
        } else {
            (0.0, false)
        };
        let run = Rect::from_two_pos(pos2(start.x, level), pos2(edge, level)).expand(1.0);
        let rise = Rect::from_two_pos(start, pos2(start.x, level)).shrink2(vec2(0.0, 2.0));
        if clear
            && start.y > level
            && (start.y - level) < 60.0 * zoom
            && !obstacles
                .iter()
                .any(|card| card.intersects(run) || card.shrink(1.0).intersects(rise))
        {
            paint::band_wire(
                painter,
                &[start, pos2(start.x, level), pos2(edge, level)],
                color,
                zoom,
            );
            return;
        }
    }
    let mut cut: Vec<Pos2> = Vec::new();
    for pair in line.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        if cut.is_empty() {
            cut.push(from);
        }
        if into.contains(to) && !into.contains(from) {
            // Where the segment meets the edge.
            let mut t = 1.0_f32;
            let delta = to - from;
            for (edge, origin, along) in [
                (into.left(), from.x, delta.x),
                (into.right(), from.x, delta.x),
                (into.top(), from.y, delta.y),
                (into.bottom(), from.y, delta.y),
            ] {
                if along.abs() > f32::EPSILON {
                    let at = (edge - origin) / along;
                    let point = from + delta * at;
                    if (0.0..=1.0).contains(&at) && into.expand(0.5).contains(point) {
                        t = t.min(at);
                    }
                }
            }
            cut.push(from + delta * t);
            break;
        }
        if into.contains(from) && cut.len() == 1 && pair == &line[..2] {
            // It starts inside (the `if` stands in the band's box): go on.
            cut.push(to);
            continue;
        }
        cut.push(to);
    }
    if cut.len() < 2 {
        return;
    }
    paint::band_wire(painter, &cut, color, zoom);
}

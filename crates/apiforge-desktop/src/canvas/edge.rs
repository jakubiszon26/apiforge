//! Off-screen relations (LAY-15): chips on the board's edge for the related
//! cards out of view, the jump to one of them and *Back*.
//!
//! While one step is selected (LAY-10) each of its sources and readers out
//! of view, and while a trace is kept (LAY-14) each end of a kept line out
//! of view, has a chip on the edge of the board towards it, labelled by its
//! role and an arrow: `Source: rooms.body.items[1].id ↑`. A card is out of
//! view while its header row is not wholly in the board's free area: inside
//! the board and clear of the minimap. More than two on one edge are one
//! chip, `↑ 3 sources`, which a click unfolds into its members until the
//! selection or the trace changes. Checks and order relations have none.
//!
//! A click on a chip goes to its card at the same zoom, the card whole in
//! the board's focus; the selection, the trace and the Inspector stay.
//! *Back*, at the top left of the board after a jump, returns to where the
//! first jump started: the camera exactly, the selection, the trace and
//! the Inspector (the folder window keeps the Inspector's state). It stays
//! until it is used or the step it returns to is removed.
//!
//! Navigation only: nothing is saved, and no card or route moves.

use super::{Camera, Card, Command, trace};
use crate::theme;
use apiforge_core::model::NodeKind;
use eframe::egui::{
    self, Color32, FontId, Painter, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2,
};
use std::collections::BTreeMap;

/// The edge of the board a chip stands on, towards its card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

impl Side {
    const ALL: [Side; 4] = [Side::Top, Side::Bottom, Side::Left, Side::Right];

    fn arrow(self) -> &'static str {
        match self {
            Side::Top => "↑",
            Side::Bottom => "↓",
            Side::Left => "←",
            Side::Right => "→",
        }
    }

    fn word(self) -> &'static str {
        match self {
            Side::Top => "above",
            Side::Bottom => "below",
            Side::Left => "to the left",
            Side::Right => "to the right",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Role {
    Source,
    Reader,
}

impl Role {
    fn label(self) -> &'static str {
        match self {
            Role::Source => "Source",
            Role::Reader => "Reader",
        }
    }
}

/// The board's free area: what is inside it and clear of the minimap is in
/// view.
#[derive(Clone, Copy)]
pub struct Free {
    pub board: Rect,
    pub minimap: Option<Rect>,
}

impl Free {
    fn holds(&self, rect: Rect) -> bool {
        self.board.contains_rect(rect)
            && self.minimap.is_none_or(|minimap| !minimap.intersects(rect))
    }

    /// Where a rectangle on screen may stand clear of the minimap.
    fn clear(&self, rect: Rect) -> bool {
        self.board.shrink(2.0).contains_rect(rect)
            && self
                .minimap
                .is_none_or(|minimap| !minimap.expand(4.0).intersects(rect))
    }
}

/// A related card out of view.
#[derive(Clone, Debug)]
pub struct Away {
    /// The card out of view, by index and id.
    card: usize,
    id: String,
    role: Role,
    /// What the chip names: `rooms.body.items[1].id`, `slot · 2 fields`.
    name: String,
    side: Side,
    /// Where its header is along the edge, on screen.
    along: f32,
    /// The card in view it is related to: the selected step, or the other
    /// end of the kept line. *Back* returns to it.
    origin: usize,
}

/// Where the first jump started: the camera, the selection and the trace.
struct Origin {
    camera: Camera,
    moved_by_user: bool,
    fitted_for: Option<Vec2>,
    selection: Vec<String>,
    wires: Vec<String>,
    anchors: Vec<(Vec<String>, trace::Anchor)>,
    /// The step it returns to, by id, and its name.
    step: String,
    name: String,
}

/// What the edge chips keep from one frame to the next.
#[derive(Default)]
pub struct Memory {
    origin: Option<Origin>,
    /// The edges whose folded chip was unfolded, and the selection and kept
    /// wires they were unfolded for.
    unfolded: Vec<Side>,
    unfolded_for: (Vec<String>, Vec<String>),
    #[cfg(test)]
    drawn: Vec<DrawnEdge>,
    #[cfg(test)]
    back: Option<(String, Rect)>,
}

/// An edge chip as the last frame drew it.
#[cfg(test)]
#[derive(Clone, Debug)]
pub(crate) struct DrawnEdge {
    /// What it says: `Source: rooms.body.items[1].id ↑`, `↑ 3 sources`.
    pub text: String,
    /// Its button's accessible name.
    pub name: String,
    pub side: Side,
    /// Its box on screen.
    pub rect: Rect,
    /// The cards it stands for: one, or the members of a folded chip.
    pub targets: Vec<String>,
}

/// One chip of an edge, before it is placed.
struct Planned {
    /// Its button's accessible name, what it says and its tooltip.
    name: String,
    text: String,
    hover: String,
    side: Side,
    /// Where along the edge it would stand, on screen.
    along: f32,
    /// The related cards it stands for, as indices into the aways.
    members: Vec<usize>,
}

const FONT: f32 = theme::FONT_LABEL;
const PAD: Vec2 = vec2(9.0, 4.0);
/// The room between a chip and the board's edge, and between chips.
const MARGIN: f32 = 8.0;
const GAP: f32 = 6.0;
/// The edge chip's teal: LAY-10's related hue, and its text a shade darker.
const OUTLINE: Color32 = super::paint::RELATED;
const INK: Color32 = Color32::from_rgb(22, 110, 110);
const FILL: Color32 = Color32::from_rgb(236, 248, 248);
const FILL_HOVER: Color32 = Color32::from_rgb(214, 239, 239);
/// *Back* is in the accent, its fill a shade darker while pointed at.
const BACK_HOVER: Color32 = Color32::from_rgb(246, 214, 194);

/// The related cards out of view: the sources and readers of the one
/// selected step, and the ends of the kept lines.
pub fn away(
    board: &trace::Board<'_>,
    selection: &[String],
    kept: &[String],
    camera: Camera,
    free: Free,
) -> Vec<Away> {
    let cards = board.cards;
    let index = |id: &str| cards.iter().position(|card| card.id == id);
    let check = |card: usize| matches!(cards[card].kind, NodeKind::Assert | NodeKind::Condition);
    let in_view = |card: usize| free.holds(camera.rect(cards[card].header_rect()));
    // Each related card and role, with the references named and the card
    // in view it is related to.
    let mut found: BTreeMap<(usize, Role), (Vec<String>, usize)> = BTreeMap::new();
    let mut note = |card: usize, role: Role, name: String, origin: usize| {
        let entry = found.entry((card, role)).or_insert((Vec::new(), origin));
        if !entry.0.contains(&name) {
            entry.0.push(name);
        }
    };
    let ends = |wire: &super::routing::BoardWire| match wire.start {
        super::routing::Start::Output { card, .. } if !wire.control => Some((card, wire.target)),
        _ => None,
    };
    let sides = |wire: &super::routing::BoardWire, source: usize, target: usize| {
        let (_, _, statement) = super::readers::texts(
            board.graph,
            board.project,
            wire,
            &cards[target].id,
            &cards[source].id,
            None,
        );
        let (left, right) = statement.split_once(" → ").unwrap_or_default();
        (left.to_owned(), right.to_owned())
    };
    if let [selected] = selection
        && let Some(selected) = index(selected)
    {
        for wire in board.wires {
            let Some((source, target)) = ends(wire) else {
                continue;
            };
            if target == selected && source != selected && !check(source) && !in_view(source) {
                note(
                    source,
                    Role::Source,
                    sides(wire, source, target).0,
                    selected,
                );
            } else if source == selected && target != selected && !check(target) && !in_view(target)
            {
                note(
                    target,
                    Role::Reader,
                    sides(wire, source, target).1,
                    selected,
                );
            }
        }
    }
    for wire in board.wires.iter().filter(|wire| kept.contains(&wire.id)) {
        let Some((source, target)) = ends(wire) else {
            continue;
        };
        let (left, right) = sides(wire, source, target);
        if !in_view(source) {
            note(source, Role::Source, left, target);
        }
        if !in_view(target) {
            note(target, Role::Reader, right, source);
        }
    }
    let mut away: Vec<Away> = found
        .into_iter()
        .map(|((card, role), (mut names, origin))| {
            // One read is named as the code writes it; several reads of one
            // step, by the step and how many.
            names.retain(|name| !name.is_empty());
            let handle = || {
                board
                    .graph
                    .nodes
                    .iter()
                    .find(|node| node.id == cards[card].id)
                    .map_or_else(|| cards[card].name.clone(), |node| node.handle.clone())
            };
            let name = match names.as_slice() {
                [] => handle(),
                [one] => one.clone(),
                _ => format!("{} · {} fields", handle(), names.len()),
            };
            let (side, along) = side_of(camera.rect(cards[card].header_rect()), free.board);
            Away {
                card,
                id: cards[card].id.clone(),
                role,
                name,
                side,
                along,
                origin,
            }
        })
        .collect();
    away.sort_by(|a, b| {
        (a.side, a.along, a.role)
            .partial_cmp(&(b.side, b.along, b.role))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    away
}

/// The edge towards a header row out of view, and where along that edge
/// it is. A header partly in the board leaves by the edge it crosses most;
/// one only under the minimap, by the bottom.
fn side_of(header: Rect, board: Rect) -> (Side, f32) {
    let middle = header.center();
    let side = if board.contains(middle) {
        [
            (Side::Top, board.top() - header.top()),
            (Side::Bottom, header.bottom() - board.bottom()),
            (Side::Left, board.left() - header.left()),
            (Side::Right, header.right() - board.right()),
        ]
        .into_iter()
        .filter(|(_, over)| *over > 0.0)
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(Side::Bottom, |(side, _)| side)
    } else {
        let off = middle - board.center();
        let x = off.x / (board.width() / 2.0).max(1.0);
        let y = off.y / (board.height() / 2.0).max(1.0);
        match (y.abs() >= x.abs(), y < 0.0, x < 0.0) {
            (true, true, _) => Side::Top,
            (true, false, _) => Side::Bottom,
            (false, _, true) => Side::Left,
            (false, _, false) => Side::Right,
        }
    };
    let along = match side {
        Side::Top | Side::Bottom => middle.x,
        Side::Left | Side::Right => middle.y,
    };
    (side, along)
}

/// The chips of each edge: one per related card, or one for all of them
/// where more than two stand on an edge that is not unfolded.
fn chips(away: &[Away], unfolded: &[Side]) -> Vec<Planned> {
    let mut chips = Vec::new();
    for side in Side::ALL {
        let members: Vec<usize> = (0..away.len())
            .filter(|&at| away[at].side == side)
            .collect();
        if members.len() > 2 && !unfolded.contains(&side) {
            let roles: Vec<Role> = members.iter().map(|&at| away[at].role).collect();
            let what = if roles.iter().all(|role| *role == Role::Source) {
                "sources"
            } else if roles.iter().all(|role| *role == Role::Reader) {
                "readers"
            } else {
                "related"
            };
            let text = format!("{} {} {what}", side.arrow(), members.len());
            let along =
                members.iter().map(|&at| away[at].along).sum::<f32>() / members.len() as f32;
            chips.push(Planned {
                name: format!("{text} (unfold)"),
                hover: format!("Show the {} {what} {}", members.len(), side.word()),
                text,
                side,
                along,
                members,
            });
            continue;
        }
        for at in members {
            let related = &away[at];
            let text = format!(
                "{}: {} {}",
                related.role.label(),
                related.name,
                side.arrow()
            );
            chips.push(Planned {
                name: format!("{}: {} (jump)", related.role.label(), related.name),
                text,
                hover: format!("Go to {}", related.name),
                side,
                along: related.along,
                members: vec![at],
            });
        }
    }
    chips
}

/// Where a box of `size` stands on `side`, as near to `along` as it can,
/// clear of `avoid` and inside the free area; rows further in when the
/// edge is full.
fn place(side: Side, along: f32, size: Vec2, free: Free, avoid: &[Rect]) -> Rect {
    let board = free.board;
    // Where it would stand on its edge, nearest its card; the search moves
    // it from there both ways.
    let start = match side {
        Side::Top | Side::Bottom => (along - size.x / 2.0)
            .min(board.right() - MARGIN - size.x)
            .max(board.left() + MARGIN),
        Side::Left | Side::Right => (along - size.y / 2.0)
            .min(board.bottom() - MARGIN - size.y)
            .max(board.top() + MARGIN),
    };
    let at = |row: usize, shift: f32| {
        let inward = MARGIN + row as f32 * (size.y + GAP);
        match side {
            Side::Top | Side::Bottom => {
                let x = (start + shift)
                    .min(board.right() - MARGIN - size.x)
                    .max(board.left() + MARGIN);
                let y = match side {
                    Side::Top => board.top() + inward,
                    _ => board.bottom() - inward - size.y,
                };
                Rect::from_min_size(pos2(x, y), size)
            }
            Side::Left | Side::Right => {
                let inward = MARGIN + row as f32 * (size.x + GAP);
                let y = (start + shift)
                    .min(board.bottom() - MARGIN - size.y)
                    .max(board.top() + MARGIN);
                let x = match side {
                    Side::Left => board.left() + inward,
                    _ => board.right() - inward - size.x,
                };
                Rect::from_min_size(pos2(x, y), size)
            }
        }
    };
    let reach = match side {
        Side::Top | Side::Bottom => board.width(),
        Side::Left | Side::Right => board.height(),
    };
    let fits = |rect: Rect| {
        free.clear(rect)
            && avoid
                .iter()
                .all(|other| !other.expand(GAP / 2.0).intersects(rect))
    };
    for row in 0..4 {
        let mut step = 0.0;
        while step <= reach {
            for shift in [step, -step] {
                let rect = at(row, shift);
                if fits(rect) {
                    return rect;
                }
            }
            step += 8.0;
        }
    }
    at(0, 0.0)
}

/// Lays the chips of each edge out in a row along it, in the order of
/// their cards, each as near its card as the row allows; a chip the row
/// cannot hold clear of `avoid` is placed on its own, further in where the
/// edge is full. Each chip placed joins `avoid`.
fn lay(planned: &[Planned], sizes: &[Vec2], free: Free, avoid: &mut Vec<Rect>) -> Vec<Rect> {
    let mut rects = vec![Rect::NOTHING; planned.len()];
    let board = free.board;
    for side in Side::ALL {
        let row: Vec<usize> = (0..planned.len())
            .filter(|&at| planned[at].side == side)
            .collect();
        let across = matches!(side, Side::Top | Side::Bottom);
        let length = |at: usize| if across { sizes[at].x } else { sizes[at].y };
        let (low, high) = if across {
            (board.left() + MARGIN, board.right() - MARGIN)
        } else {
            (board.top() + MARGIN, board.bottom() - MARGIN)
        };
        // Each as near its card as the one before it allows, then pushed
        // back from the far end.
        let mut starts: Vec<f32> = Vec::with_capacity(row.len());
        let mut cursor = low;
        for &at in &row {
            let start = (planned[at].along - length(at) / 2.0).max(cursor);
            starts.push(start);
            cursor = start + length(at) + GAP;
        }
        let mut limit = high;
        for (index, &at) in row.iter().enumerate().rev() {
            starts[index] = starts[index].min(limit - length(at));
            limit = starts[index] - GAP;
        }
        let mut pending: Vec<usize> = Vec::new();
        for (index, &at) in row.iter().enumerate() {
            let size = sizes[at];
            let min = if across {
                pos2(
                    starts[index],
                    match side {
                        Side::Top => board.top() + MARGIN,
                        _ => board.bottom() - MARGIN - size.y,
                    },
                )
            } else {
                pos2(
                    match side {
                        Side::Left => board.left() + MARGIN,
                        _ => board.right() - MARGIN - size.x,
                    },
                    starts[index],
                )
            };
            let rect = Rect::from_min_size(min, size);
            let clear = starts[index] >= low
                && free.clear(rect)
                && avoid
                    .iter()
                    .all(|other| !other.expand(GAP / 2.0).intersects(rect));
            if clear {
                rects[at] = rect;
            } else {
                pending.push(at);
            }
        }
        avoid.extend(
            row.iter()
                .map(|&at| rects[at])
                .filter(|rect| rect.is_positive()),
        );
        for at in pending {
            let rect = place(side, planned[at].along, sizes[at], free, avoid);
            rects[at] = rect;
            avoid.push(rect);
        }
    }
    rects
}

fn pill(
    painter: &Painter,
    rect: Rect,
    galley: std::sync::Arc<egui::Galley>,
    look: (Color32, Color32, Color32),
) {
    let (fill, outline, ink) = look;
    painter.rect(
        rect,
        rect.height() / 2.0,
        fill,
        Stroke::new(1.0, outline),
        StrokeKind::Inside,
    );
    painter.galley(rect.min + PAD, galley, ink);
}

/// What the board gives the edge chips this frame.
pub struct Frame<'a> {
    pub cards: &'a [Card],
    pub selection: &'a [String],
    pub viewport: Rect,
    /// Where a card goes to: the board's focus.
    pub focus: Rect,
    pub free: Free,
    /// What no chip and no *Back* covers: the selected cards, the zoom bar.
    pub avoid: &'a [Rect],
}

impl super::Canvas {
    /// Forgets *Back* once the step it returns to is gone, and the unfolded
    /// chips once the selection or the trace changes.
    pub(super) fn edges_follow(
        &mut self,
        graph: &apiforge_core::format::Graph,
        selection: &[String],
    ) {
        if self
            .edges
            .origin
            .as_ref()
            .is_some_and(|origin| !graph.nodes.iter().any(|node| node.id == origin.step))
        {
            self.edges.origin = None;
        }
        let now = (selection.to_vec(), self.selected_wires.clone());
        if self.edges.unfolded_for != now {
            self.edges.unfolded.clear();
            self.edges.unfolded_for = now;
        }
    }

    /// Whether *Back* is shown: the folder window keeps the Inspector's
    /// state for it while it is.
    pub fn has_back(&self) -> bool {
        self.edges.origin.is_some()
    }

    /// Forgets *Back* and the unfolded chips: the board shows another
    /// context.
    pub(super) fn edges_reset(&mut self) {
        self.edges.origin = None;
        self.edges.unfolded.clear();
    }

    /// Lays out, draws and names *Back* and the edge chips, and handles a
    /// click on them. Returns their boxes on screen, which a kept statement
    /// keeps clear of.
    pub(super) fn show_edges(
        &mut self,
        ui: &mut egui::Ui,
        painter: &Painter,
        away: &[Away],
        frame: &Frame<'_>,
        commands: &mut Vec<Command>,
    ) -> Vec<Rect> {
        let mut placed: Vec<Rect> = Vec::new();
        let mut avoid: Vec<Rect> = frame.avoid.to_vec();
        #[cfg(test)]
        {
            self.edges.drawn.clear();
            self.edges.back = None;
        }
        let mut back = false;
        if let Some(origin) = &self.edges.origin {
            let text = format!("← Back to {}", origin.name);
            let name = format!("Back to {}", origin.name);
            let galley =
                painter.layout_no_wrap(text.clone(), FontId::proportional(FONT), theme::ACCENT);
            let size = galley.size() + PAD * 2.0;
            let start = frame.free.board.left() + MARGIN + size.x / 2.0;
            let rect = place(Side::Top, start, size, frame.free, &avoid);
            let response = ui
                .interact(rect, ui.id().with("edge-back"), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Return to where you were before the jump");
            response
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
            let fill = if response.hovered() {
                BACK_HOVER
            } else {
                theme::ACCENT_SOFT
            };
            pill(painter, rect, galley, (fill, theme::ACCENT, theme::ACCENT));
            back = response.clicked();
            placed.push(rect);
            avoid.push(rect);
            #[cfg(test)]
            {
                self.edges.back = Some((text, rect));
            }
        }
        let mut jump: Option<usize> = None;
        let planned = chips(away, &self.edges.unfolded);
        let galleys: Vec<_> = planned
            .iter()
            .map(|chip| painter.layout_no_wrap(chip.text.clone(), FontId::proportional(FONT), INK))
            .collect();
        let sizes: Vec<Vec2> = galleys
            .iter()
            .map(|galley| galley.size() + PAD * 2.0)
            .collect();
        let rects = lay(&planned, &sizes, frame.free, &mut avoid);
        for ((chip, galley), rect) in planned.into_iter().zip(galleys).zip(rects) {
            let Planned {
                name,
                hover,
                side,
                members,
                ..
            } = chip;
            let targets: Vec<&str> = members.iter().map(|&at| away[at].id.as_str()).collect();
            let response = ui
                .interact(
                    rect,
                    ui.id().with(("edge-chip", side, &targets)),
                    Sense::click(),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(hover);
            response
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
            let fill = if response.hovered() { FILL_HOVER } else { FILL };
            #[cfg(test)]
            let text = galley.text().to_owned();
            pill(painter, rect, galley, (fill, OUTLINE, INK));
            if response.clicked() {
                match members.as_slice() {
                    [one] => jump = Some(*one),
                    _ => {
                        if !self.edges.unfolded.contains(&side) {
                            self.edges.unfolded.push(side);
                        }
                        ui.ctx().request_repaint();
                    }
                }
            }
            placed.push(rect);
            #[cfg(test)]
            self.edges.drawn.push(DrawnEdge {
                text,
                name,
                side,
                rect,
                targets: targets.iter().map(|id| (*id).to_owned()).collect(),
            });
        }
        if back {
            self.go_back(commands);
            ui.ctx().request_repaint();
        } else if let Some(at) = jump {
            self.jump(&away[at], frame, commands);
            ui.ctx().request_repaint();
        }
        placed
    }

    /// Goes to the card of `related` at the same zoom, the card whole in
    /// the focus; the first jump remembers where it started.
    fn jump(&mut self, related: &Away, frame: &Frame<'_>, commands: &mut Vec<Command>) {
        let Some(card) = frame.cards.get(related.card) else {
            return;
        };
        if self.edges.origin.is_none() {
            let origin = &frame.cards[related.origin];
            self.edges.origin = Some(Origin {
                camera: self.camera,
                moved_by_user: self.camera_moved_by_user,
                fitted_for: self.fitted_for,
                selection: frame.selection.to_vec(),
                wires: self.selected_wires.clone(),
                anchors: self.trace.anchors(),
                step: origin.id.clone(),
                name: origin.name.clone(),
            });
            commands.push(Command::Jumped);
        }
        let zoom = self.camera.zoom;
        let rect = card.rect();
        let size = rect.size() * zoom;
        let focus = frame.focus;
        let centre = pos2(
            if size.x <= focus.width() {
                focus.center().x
            } else {
                focus.left() + size.x / 2.0
            },
            if size.y <= focus.height() {
                focus.center().y
            } else {
                focus.top() + size.y / 2.0
            },
        );
        self.camera.pan = (centre - frame.viewport.min) - rect.center().to_vec2() * zoom;
        self.camera_moved_by_user = true;
        self.fitted_for = None;
    }

    /// Returns to where the first jump started.
    fn go_back(&mut self, commands: &mut Vec<Command>) {
        let Some(origin) = self.edges.origin.take() else {
            return;
        };
        self.camera = origin.camera;
        self.camera_moved_by_user = origin.moved_by_user;
        self.fitted_for = origin.fitted_for;
        self.selected_wires = origin.wires;
        self.trace.restore(origin.anchors);
        // The step returned to is where it was: selecting it again pans
        // nothing (UX-18).
        self.kept_in_view = match origin.selection.as_slice() {
            [one] => Some(one.clone()),
            _ => None,
        };
        self.edges.unfolded.clear();
        commands.push(Command::Back(origin.selection));
    }

    /// The edge chips the last frame drew.
    #[cfg(test)]
    pub(crate) fn debug_edge_chips(&self) -> Vec<DrawnEdge> {
        self.edges.drawn.clone()
    }

    /// *Back* as the last frame drew it: its text and its box on screen.
    #[cfg(test)]
    pub(crate) fn debug_back(&self) -> Option<(String, Rect)> {
        self.edges.back.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOARD: Rect = Rect {
        min: pos2(0.0, 0.0),
        max: pos2(1000.0, 600.0),
    };

    #[test]
    fn a_header_leaves_by_the_edge_towards_it() {
        let header = |x: f32, y: f32| Rect::from_min_size(pos2(x, y), vec2(200.0, 30.0));
        assert_eq!(side_of(header(400.0, -300.0), BOARD).0, Side::Top);
        assert_eq!(side_of(header(400.0, 900.0), BOARD).0, Side::Bottom);
        assert_eq!(side_of(header(-900.0, 300.0), BOARD).0, Side::Left);
        assert_eq!(side_of(header(1500.0, 300.0), BOARD).0, Side::Right);
        // Partly out: by the edge it crosses.
        assert_eq!(side_of(header(400.0, -10.0), BOARD).0, Side::Top);
        assert_eq!(side_of(header(900.0, 300.0), BOARD).0, Side::Right);
    }

    #[test]
    fn a_chip_keeps_clear_of_what_it_avoids() {
        let free = Free {
            board: BOARD,
            minimap: Some(Rect::from_min_size(pos2(850.0, 500.0), vec2(136.0, 88.0))),
        };
        let size = vec2(200.0, 22.0);
        let selected = Rect::from_min_size(pos2(300.0, 0.0), vec2(300.0, 200.0));
        let rect = place(Side::Top, 450.0, size, free, &[selected]);
        assert!(!rect.intersects(selected));
        assert!(BOARD.contains_rect(rect));
        let next = place(Side::Top, 450.0, size, free, &[selected, rect]);
        assert!(!next.intersects(rect) && !next.intersects(selected));
        let corner = place(Side::Bottom, 950.0, size, free, &[]);
        assert!(!corner.intersects(free.minimap.unwrap()));
    }
}

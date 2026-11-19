//! MOCK (DIAG-06, #391, with LAY-45, #468): the chain drawn side to side
//! and the feedback of the gestures that keep it, painted over the real
//! board for the design-review views. Nothing here edits a document: a
//! scene says which lines, bands and gesture marks to draw on the cards
//! the board has laid out. Throwaway; the branch is kept for the captures.

use super::Camera;
use super::layout::{Card, WIDTH};
use super::shape;
use crate::theme;
use apiforge_core::graph::{FALSE_PORT, TRUE_PORT};
use apiforge_core::model::NodeKind;
use eframe::egui::{
    Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2, pos2, vec2,
};
use std::sync::Mutex;

/// Where the order sockets sit on a card's sides: on its title line, clear
/// of the badge above and of the first data socket below.
const SOCKET_Y: f32 = 22.0;
/// How far a line to another row runs out of a card before it turns.
const OUT: f32 = 40.0;
/// How far left of the next row's first card it comes down.
const IN: f32 = 26.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    /// A line of the chain at rest.
    Plain,
    /// A line the gesture is about to replace.
    Fading,
}

/// A line of the chain, by the names of its cards.
#[derive(Clone, Debug)]
pub struct Link {
    pub from: String,
    pub to: String,
    pub look: Look,
}

/// The braces of a branch as a band (LAY-13, approved): its steps, the `if`
/// and which of its sockets leads in, and the step the chain goes on with.
#[derive(Clone, Debug)]
pub struct Band {
    pub condition: String,
    pub yes: bool,
    pub steps: Vec<String>,
    pub then: Option<String>,
    /// Room kept free at the band's end, in cards: where a step may land.
    pub room: f32,
}

#[derive(Clone, Debug)]
pub enum Target {
    /// The order socket of a card.
    Card(String),
    /// A point of the board, as an offset from a card's *Next* socket.
    Board(String, Vec2),
}

/// A wire carried from a *Next* socket.
#[derive(Clone, Debug)]
pub struct Carry {
    pub from: String,
    pub to: Target,
    pub caption: Vec<String>,
    /// Where the caption stands, from the pointer.
    pub caption_at: Vec2,
    pub refused: bool,
}

/// A block carried from the Library over a line of the chain.
#[derive(Clone, Debug)]
pub struct Drop {
    pub block: String,
    pub after: String,
    pub before: String,
    pub caption: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub on: bool,
    pub links: Vec<Link>,
    pub bands: Vec<Band>,
    pub carry: Option<Carry>,
    pub drop: Option<Drop>,
    /// What the board says after an edit, with *Undo*.
    pub notice: Option<String>,
    /// Cards moved from where Tidy up put them, by name.
    pub moved: Vec<(String, Vec2)>,
    /// The pointer over a line: the place a step can be added.
    pub plus: Option<(String, String)>,
}

static SCENE: Mutex<Option<Scene>> = Mutex::new(None);

pub fn set(scene: Scene) {
    *SCENE.lock().unwrap_or_else(|poison| poison.into_inner()) = Some(scene);
}

pub fn clear() {
    *SCENE.lock().unwrap_or_else(|poison| poison.into_inner()) = None;
}

fn scene() -> Option<Scene> {
    SCENE
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .clone()
        .filter(|scene| scene.on)
}

pub fn on() -> bool {
    scene().is_some()
}

/// Moves the cards a scene places by hand.
pub(super) fn place(cards: &mut [Card]) {
    let Some(scene) = scene() else { return };
    for (name, offset) in &scene.moved {
        if let Some(card) = cards.iter_mut().find(|card| &card.name == name) {
            *card = card.translated(*offset);
        }
    }
}

fn card<'a>(cards: &'a [Card], name: &str) -> Option<&'a Card> {
    cards.iter().find(|card| card.name == name)
}

fn next_socket(card: &Card) -> Pos2 {
    card.origin + vec2(WIDTH, SOCKET_Y)
}

fn order_socket(card: &Card) -> Pos2 {
    card.origin + vec2(0.0, SOCKET_Y)
}

fn dashed(painter: &Painter, line: &[Pos2], stroke: Stroke, zoom: f32) {
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
            let step = remaining.min(length - position).max(0.05);
            if phase < on {
                painter.line_segment(
                    [
                        from + direction * position,
                        from + direction * (position + step),
                    ],
                    stroke,
                );
            }
            position += step;
        }
        travelled += length;
    }
}

fn head(painter: &Painter, tip: Pos2, direction: Vec2, color: Color32, zoom: f32) {
    let size = 9.0 * zoom.max(0.6);
    let normal = vec2(-direction.y, direction.x);
    painter.add(Shape::convex_polygon(
        vec![
            tip,
            tip - direction * size + normal * size * 0.55,
            tip - direction * size - normal * size * 0.55,
        ],
        color,
        Stroke::NONE,
    ));
}

fn arrow(painter: &Painter, line: &[Pos2], color: Color32, width: f32, zoom: f32) {
    if line.len() < 2 {
        return;
    }
    // The head stops short of the socket it points at.
    let mut line = line.to_vec();
    let last = line.len() - 1;
    let direction = (line[last] - line[last - 1]).normalized();
    line[last] -= direction * ((4.5 * zoom).max(1.25) + 2.0);
    let tip = line[last];
    line[last] -= direction * 5.0 * zoom.max(0.6);
    dashed(painter, &line, Stroke::new(width, color), zoom);
    head(painter, tip, direction, color, zoom);
}

fn socket(painter: &Painter, center: Pos2, zoom: f32, color: Color32) {
    let radius = (4.5 * zoom).max(1.25);
    painter.rect(
        Rect::from_center_size(center, Vec2::splat(radius * 2.0)),
        1.5,
        theme::WHITE,
        Stroke::new((1.0 * zoom).clamp(0.6, 1.0), color),
        StrokeKind::Inside,
    );
}

/// The route of a line of the chain in world units: straight inside a row,
/// otherwise out of the card, through the gap under its row and into the
/// next card from its left.
fn route(cards: &[Card], from: &Card, to: &Card) -> Vec<Pos2> {
    let (start, end) = (next_socket(from), order_socket(to));
    if (start.y - end.y).abs() < 1.0 && end.x > start.x {
        return vec![start, end];
    }
    let row_bottom = cards
        .iter()
        .filter(|card| (card.origin.y - from.origin.y).abs() < 40.0)
        .map(|card| card.rect().bottom())
        .fold(from.rect().bottom(), f32::max);
    let gap = if to.origin.y > row_bottom {
        (row_bottom + to.origin.y) / 2.0
    } else {
        row_bottom + 60.0
    };
    vec![
        start,
        pos2(start.x + OUT, start.y),
        pos2(start.x + OUT, gap),
        pos2(end.x - IN, gap),
        pos2(end.x - IN, end.y),
        end,
    ]
}

fn on_screen(camera: Camera, line: &[Pos2]) -> Vec<Pos2> {
    line.iter().map(|point| camera.to_screen(*point)).collect()
}

fn band_rect(cards: &[Card], band: &Band) -> Option<Rect> {
    let mut rect: Option<Rect> = None;
    for name in &band.steps {
        let card = card(cards, name)?;
        rect = Some(rect.map_or(card.rect(), |all| all.union(card.rect())));
    }
    let mut rect = rect?;
    rect.max.x += band.room * (WIDTH + 100.0);
    Some(Rect::from_min_max(
        rect.min - vec2(44.0, 44.0),
        rect.max + vec2(44.0, 28.0),
    ))
}

fn pill(painter: &Painter, at: Pos2, text: &str, zoom: f32, ink: Color32, border: Color32) -> Rect {
    let font = FontId::proportional((12.0 * zoom).max(10.0));
    let galley = painter.layout_no_wrap(text.to_owned(), font, ink);
    let rect = Rect::from_min_size(
        pos2(at.x, at.y - galley.size().y / 2.0 - 2.0),
        galley.size() + vec2(12.0, 4.0),
    );
    painter.rect(
        rect,
        rect.height() / 2.0,
        theme::WHITE,
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );
    painter.galley(rect.min + vec2(6.0, 2.0), galley, ink);
    rect
}

/// Under the cards: the bands and the lines of the chain.
pub(super) fn under(painter: &Painter, cards: &[Card], camera: Camera) {
    let Some(scene) = scene() else { return };
    let zoom = camera.zoom;
    for band in &scene.bands {
        let (Some(world), Some(condition)) = (band_rect(cards, band), card(cards, &band.condition))
        else {
            continue;
        };
        let rect = camera.rect(world);
        let color = if band.yes {
            theme::BRANCH_YES
        } else {
            theme::BRANCH_NO
        };
        painter.rect(
            rect,
            6.0 * zoom,
            Color32::from_rgb(236, 237, 240),
            Stroke::new(1.0, Color32::from_rgb(200, 204, 211)),
            StrokeKind::Inside,
        );
        painter.text(
            rect.left_top() + vec2(14.0, 9.0) * zoom,
            Align2::LEFT_TOP,
            format!(
                "{} → {}",
                band.condition,
                if band.yes { "Yes" } else { "No" }
            ),
            FontId::proportional((13.0 * zoom).max(9.0)),
            theme::MUTED,
        );
        // One line from the `if` into the band.
        let port = if band.yes { TRUE_PORT } else { FALSE_PORT };
        if let Some(start) = condition.output_anchor(port) {
            let x = start.x.clamp(world.left() + 30.0, world.right() - 30.0);
            let middle = (start.y + world.top()) / 2.0;
            let line = if (x - start.x).abs() < 1.0 {
                vec![start, pos2(x, world.top())]
            } else {
                vec![
                    start,
                    pos2(start.x, middle),
                    pos2(x, middle),
                    pos2(x, world.top()),
                ]
            };
            arrow(painter, &on_screen(camera, &line), color, 1.6, zoom);
        }
        // Into the first step of the braces.
        if let Some(first) = band.steps.first().and_then(|name| card(cards, name)) {
            let end = order_socket(first);
            let line = [pos2(world.left(), end.y), end];
            arrow(painter, &on_screen(camera, &line), color, 1.6, zoom);
        }
    }
    for link in &scene.links {
        let (Some(from), Some(to)) = (card(cards, &link.from), card(cards, &link.to)) else {
            continue;
        };
        let line = on_screen(camera, &route(cards, from, to));
        let (color, width) = match link.look {
            Look::Plain => (theme::CONTROL_WIRE, 1.6),
            Look::Fading => (theme::CONTROL_WIRE.gamma_multiply(0.35), 1.6),
        };
        arrow(painter, &line, color, width, zoom);
    }
}

fn caption(painter: &Painter, at: Pos2, lines: &[String], refused: bool, within: Rect) {
    let strong = FontId::proportional(13.0);
    let plain = FontId::proportional(12.0);
    let galleys: Vec<_> = lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let (font, ink) = if index == 0 {
                (
                    strong.clone(),
                    if refused { theme::DANGER } else { theme::TEXT },
                )
            } else {
                (plain.clone(), theme::MUTED)
            };
            painter.layout_no_wrap(line.clone(), font, ink)
        })
        .collect();
    let width = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max);
    let height: f32 = galleys.iter().map(|g| g.size().y + 3.0).sum();
    let mut rect = Rect::from_min_size(at, vec2(width + 20.0, height + 13.0));
    if rect.right() > within.right() - 8.0 {
        rect = rect.translate(vec2(within.right() - 8.0 - rect.right(), 0.0));
    }
    if rect.bottom() > within.bottom() - 8.0 {
        rect = rect.translate(vec2(0.0, at.y - 44.0 - rect.bottom()));
    }
    painter.rect(
        rect.translate(vec2(0.0, 2.0)),
        7.0,
        Color32::from_black_alpha(22),
        Stroke::NONE,
        StrokeKind::Inside,
    );
    painter.rect(
        rect,
        7.0,
        if refused {
            Color32::from_rgb(253, 236, 232)
        } else {
            theme::WHITE
        },
        Stroke::new(
            1.0,
            if refused {
                theme::DANGER
            } else {
                theme::BORDER
            },
        ),
        StrokeKind::Inside,
    );
    let mut y = rect.top() + 8.0;
    for galley in galleys {
        let step = galley.size().y + 3.0;
        painter.galley(pos2(rect.left() + 10.0, y), galley, theme::TEXT);
        y += step;
    }
}

/// The pointer, which a capture does not have.
fn pointer(painter: &Painter, at: Pos2) {
    let shape = [
        vec2(0.0, 0.0),
        vec2(0.0, 16.0),
        vec2(4.2, 12.4),
        vec2(7.0, 18.4),
        vec2(9.4, 17.3),
        vec2(6.7, 11.5),
        vec2(11.8, 11.5),
    ];
    let points: Vec<Pos2> = shape.iter().map(|offset| at + *offset).collect();
    // Concave: drawn as a fan of triangles from the tip.
    for pair in points[1..].windows(2) {
        painter.add(Shape::convex_polygon(
            vec![points[0], pair[0], pair[1]],
            Color32::from_rgb(24, 24, 27),
            Stroke::NONE,
        ));
    }
    painter.add(Shape::closed_line(points, Stroke::new(1.0, theme::WHITE)));
}

fn curve(painter: &Painter, from: Pos2, to: Pos2, color: Color32, zoom: f32) {
    let reach = ((to.x - from.x).abs() * 0.5).max(40.0 * zoom);
    let points = [from, from + vec2(reach, 0.0), to - vec2(reach, 0.0), to];
    let samples: Vec<Pos2> = (0..=48)
        .map(|step| super::paint::bezier(points, step as f32 / 48.0))
        .collect();
    dashed(painter, &samples, Stroke::new(2.2, color), zoom);
    head(painter, to, vec2(1.0, 0.0), color, zoom);
}

/// Over the cards: the order sockets, the ends of the branches and the
/// gesture in progress.
pub(super) fn over(painter: &Painter, cards: &[Card], camera: Camera, board: Rect) {
    let Some(scene) = scene() else { return };
    let zoom = camera.zoom;
    for card in cards.iter().filter(|card| shape::on_spine(card.kind)) {
        socket(
            painter,
            camera.to_screen(next_socket(card)),
            zoom,
            theme::SOCKET,
        );
        if card.kind != NodeKind::Start {
            socket(
                painter,
                camera.to_screen(order_socket(card)),
                zoom,
                theme::SOCKET,
            );
        }
    }
    for band in &scene.bands {
        let (Some(last), Some(then)) = (
            band.steps.last().and_then(|name| card(cards, name)),
            band.then.as_ref(),
        ) else {
            continue;
        };
        // The Overview zoom has no room for it.
        if zoom < 0.4 {
            continue;
        }
        let at = camera.to_screen(next_socket(last) + vec2(18.0, 0.0));
        pill(
            painter,
            at,
            &format!("↳ {then}"),
            zoom,
            theme::MUTED,
            theme::BORDER,
        );
    }
    if let Some((from, to)) = &scene.plus
        && let (Some(from), Some(to)) = (card(cards, from), card(cards, to))
    {
        let line = on_screen(camera, &route(cards, from, to));
        arrow(painter, &line, theme::ACCENT, 2.0, zoom);
        let middle = line[0] + (line[1] - line[0]) / 2.0;
        plus(painter, middle);
        pointer(painter, middle + vec2(3.0, 4.0));
    }
    if let Some(drop) = &scene.drop
        && let (Some(after), Some(before)) = (card(cards, &drop.after), card(cards, &drop.before))
    {
        let line = on_screen(camera, &route(cards, after, before));
        arrow(painter, &line, theme::ACCENT, 2.0, zoom);
        let middle = line[0] + (line[1] - line[0]) / 2.0;
        // The place: a bar across the line, as a text cursor between words.
        painter.line_segment(
            [
                middle - vec2(0.0, 30.0 * zoom),
                middle + vec2(0.0, 30.0 * zoom),
            ],
            Stroke::new(3.0, theme::ACCENT),
        );
        plus(painter, middle);
        let at = middle + vec2(16.0, 30.0);
        let carried = pill(
            painter,
            at + vec2(14.0, 26.0),
            &drop.block,
            1.1,
            theme::TEXT,
            theme::ACCENT,
        );
        pointer(painter, at);
        caption(
            painter,
            carried.left_bottom() + vec2(0.0, 8.0),
            &drop.caption,
            false,
            board,
        );
    }
    if let Some(carry) = &scene.carry
        && let Some(from) = card(cards, &carry.from)
    {
        let start = camera.to_screen(next_socket(from));
        let (end, snapped) = match &carry.to {
            Target::Card(name) => match card(cards, name) {
                Some(target) => (camera.to_screen(order_socket(target)), true),
                None => return,
            },
            Target::Board(name, offset) => match card(cards, name) {
                Some(anchor) => (camera.to_screen(next_socket(anchor) + *offset), false),
                None => return,
            },
        };
        let color = if carry.refused {
            theme::DANGER
        } else {
            theme::ACCENT
        };
        socket(painter, start, zoom, color);
        curve(painter, start, end, color, zoom);
        if snapped {
            super::paint::port_ring(painter, end, color, zoom);
            if carry.refused {
                let radius = (7.0 * zoom).max(5.0);
                painter.line_segment(
                    [
                        end + vec2(-radius, radius) * 0.7,
                        end + vec2(radius, -radius) * 0.7,
                    ],
                    Stroke::new(2.0, color),
                );
            }
        }
        pointer(painter, end + vec2(2.0, 3.0));
        caption(
            painter,
            end + carry.caption_at,
            &carry.caption,
            carry.refused,
            board,
        );
    }
    if let Some(text) = &scene.notice {
        notice(painter, board, text);
    }
}

fn plus(painter: &Painter, at: Pos2) {
    painter.circle(at, 9.0, theme::ACCENT, Stroke::new(1.5, theme::WHITE));
    let arm = 4.5;
    for offset in [vec2(arm, 0.0), vec2(0.0, arm)] {
        painter.line_segment([at - offset, at + offset], Stroke::new(1.8, theme::WHITE));
    }
}

/// What the board says after an edit, where it shows a refusal today.
fn notice(painter: &Painter, board: Rect, text: &str) {
    let font = FontId::proportional(13.0);
    let message = painter.layout_no_wrap(text.to_owned(), font.clone(), theme::TEXT);
    let undo = painter.layout_no_wrap("Undo".to_owned(), font, theme::ACCENT);
    let width = message.size().x + undo.size().x + 52.0;
    let rect = Rect::from_center_size(
        pos2(board.center().x, board.top() + 64.0),
        vec2(width, 34.0),
    );
    painter.rect(
        rect.translate(vec2(0.0, 2.0)),
        8.0,
        Color32::from_black_alpha(22),
        Stroke::NONE,
        StrokeKind::Inside,
    );
    painter.rect(
        rect,
        8.0,
        theme::WHITE,
        Stroke::new(1.0, theme::BORDER),
        StrokeKind::Inside,
    );
    let y = rect.center().y - message.size().y / 2.0;
    painter.galley(pos2(rect.left() + 14.0, y), message.clone(), theme::TEXT);
    let x = rect.left() + 14.0 + message.size().x + 14.0;
    painter.line_segment(
        [pos2(x, rect.top() + 8.0), pos2(x, rect.bottom() - 8.0)],
        Stroke::new(1.0, theme::BORDER),
    );
    painter.galley(pos2(x + 12.0, y), undo, theme::ACCENT);
}

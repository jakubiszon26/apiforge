//! Wire and input tracing (LAY-14): which field of which step feeds which
//! input of which step.
//!
//! Pointing at a drawn line lights it, rings the port at each of its ends
//! and states it (`valid_cart.value.id → checkout.body.cartId`) in a box at
//! the pointer, as a chip's statement is shown (decision 1 = A). Pointing at
//! an input row states every value that feeds it, the lines and the chips
//! alike. A click on a line or on a chip keeps it: its statement and its
//! rings stay when the pointer leaves, and the cards not at its ends and the
//! other lines dim as for a selected step (LAY-10), until Esc or a click on
//! the empty board; pointing alone dims nothing (2 = A). A click on an input
//! row still selects the card (3 = A).
//!
//! The kept wires are the board's selected wires, so Delete still removes
//! them and everything that ends a wire's selection ends its trace. Only
//! data wires are traced: an ordering line already says `after …`.
//!
//! A reader count's list of the fields read (LAY-18) stands in the same box:
//! at the count while it is pointed at, and while its port's wires are drawn
//! by a click on it.
//!
//! A kept statement ends with *Change source* where the Inspector can read
//! the field from another step (LAY-42): a click selects the reader and opens
//! the Inspector at that field.
//!
//! Rendering only: nothing is saved, and no card, route or camera moves.

use super::{
    Camera,
    layout::{Card, Chip},
    readers,
    routing::{BoardWire, Start},
};
use crate::theme;
use apiforge_core::format::{Graph, Project};
use eframe::egui::{self, Color32, Painter, Pos2, Rect, Sense, Ui, pos2, vec2};
use std::collections::{HashMap, HashSet};

/// Where a kept statement stands.
#[derive(Clone, Copy, Debug)]
pub enum Anchor {
    /// Where the line was clicked, on the board.
    Board(Pos2),
    /// Above the chip that was clicked.
    Chip,
}

/// What tracing keeps from one frame to the next.
#[derive(Default)]
pub struct Memory {
    /// The data wires the last frame drew, in board coordinates: what the
    /// pointer is tested against.
    routes: Vec<(String, Vec<Pos2>)>,
    /// The wires kept by each click, with where its statement stands.
    kept: Vec<(Vec<String>, Anchor)>,
    /// The statements the last frame showed, with their boxes on screen.
    #[cfg(test)]
    boxes: Vec<(String, Rect)>,
    /// The board's area on screen in the last frame.
    #[cfg(test)]
    board: Option<Rect>,
}

/// The board as tracing reads it.
pub struct Board<'a> {
    pub graph: &'a Graph,
    pub project: &'a Project,
    pub cards: &'a [Card],
    pub chips: &'a [Chip],
    /// Every wire of the board, the ones chips stand for included.
    pub wires: &'a [BoardWire],
}

/// A traced wire: what is at its ends and what it says.
struct Traced {
    source: String,
    target: String,
    statement: String,
    /// The field it goes into, when the Inspector can change its source.
    field: Option<Field>,
}

/// A field whose source *Change source* opens in the Inspector (LAY-42):
/// the reading step, its input, the path in the input, and the field as the
/// statement names it (`second.body.roomId`).
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub reader: String,
    pub input: String,
    pub path: Vec<String>,
    pub name: String,
}

/// What this frame traces.
pub struct Trace {
    /// The wires pointed at, through their line or their input row.
    pointed: Vec<String>,
    /// The data wires kept by a click.
    kept: HashSet<String>,
    /// The cards at the ends of the kept wires.
    ends: HashSet<String>,
    wires: HashMap<String, Traced>,
    /// Every data wire of the board.
    data: HashSet<String>,
}

/// The input row under `pointer` that a data wire of `wires` goes into: its
/// card, its input and the row on screen.
pub fn pointed_row(
    cards: &[Card],
    wires: &[BoardWire],
    camera: Camera,
    pointer: Pos2,
) -> Option<(String, String, Rect)> {
    wires.iter().filter(|wire| !wire.control).find_map(|wire| {
        let card = cards.get(wire.target)?;
        let index = card.inputs.iter().position(|row| row.name == wire.input)?;
        let row = card.input_row(index);
        (row != Rect::NOTHING && camera.rect(row).contains(pointer))
            .then(|| (card.id.clone(), wire.input.clone(), camera.rect(row)))
    })
}

/// Where the statement of something pointed at stands: the box goes over
/// the pointer, or under it where there is no room above.
pub fn at_pointer(pointer: Pos2) -> Rect {
    Rect::from_min_max(pointer - vec2(0.0, 6.0), pointer + vec2(0.0, 20.0))
}

impl Memory {
    /// The data line within reach of `pointer` in the last frame, unless the
    /// pointer is over something in `over` (a card, a chip, a count).
    pub fn pointed(
        &self,
        camera: Camera,
        pointer: Pos2,
        mut over: impl Iterator<Item = Rect>,
    ) -> Option<String> {
        if over.any(|rect| rect.contains(pointer)) {
            return None;
        }
        super::hit_wire(&self.routes, camera, pointer)
    }

    /// Remembers the data wires this frame drew, for the next frame's
    /// pointer.
    pub fn remember<'a>(
        &mut self,
        trace: &Trace,
        drawn: impl IntoIterator<Item = &'a (String, Vec<Pos2>)>,
    ) {
        self.routes = drawn
            .into_iter()
            .filter(|(id, _)| trace.data.contains(id))
            .cloned()
            .collect();
    }

    /// A click on a line kept `wires` (`selected` already says which):
    /// its statement stands at `at`. With `add` (Ctrl+click) the others
    /// stay.
    pub fn anchor(&mut self, wires: Vec<String>, at: Anchor, add: bool) {
        if !add {
            self.kept.clear();
        }
        self.kept.retain(|(kept, _)| *kept != wires);
        self.kept.push((wires, at));
    }

    /// Where the statement of each click kept stands, for *Back* to restore
    /// (LAY-15).
    pub fn anchors(&self) -> Vec<(Vec<String>, Anchor)> {
        self.kept.clone()
    }

    /// Puts back the statements' places `anchors` remembered (LAY-15).
    pub fn restore(&mut self, anchors: Vec<(Vec<String>, Anchor)>) {
        self.kept = anchors;
    }

    /// A click on a chip keeps its wires, or with `add` (Ctrl+click) adds
    /// them to the wires kept, or takes them away when they are all kept.
    pub fn keep_chip(&mut self, selected: &mut Vec<String>, wires: &[String], add: bool) {
        if !add {
            *selected = wires.to_vec();
        } else if wires.iter().all(|wire| selected.contains(wire)) {
            selected.retain(|wire| !wires.contains(wire));
        } else {
            for wire in wires {
                if !selected.contains(wire) {
                    selected.push(wire.clone());
                }
            }
        }
        self.anchor(wires.to_vec(), Anchor::Chip, add);
    }

    /// Paints the statement of what is pointed at (`pointed`, from a chip, a
    /// count, a row or a line), the statement of each click kept and the list
    /// of the count clicked (`listed`, LAY-18), every box whole inside
    /// `board` and clear of `taken`: *Back* and the edge chips (LAY-15).
    /// With `editable`, a kept statement's line ends with *Change source*
    /// where its field allows it, on `ui`; returns the field whose button
    /// was clicked (LAY-42).
    #[allow(clippy::too_many_arguments)]
    pub fn paint(
        &mut self,
        ui: &Ui,
        painter: &Painter,
        board: Rect,
        camera: Camera,
        trace: &Trace,
        pointed: Option<&(String, Rect)>,
        listed: Option<&(String, Rect)>,
        tags: &[readers::Tag],
        laid: &[readers::Laid],
        taken: &[Rect],
        editable: bool,
    ) -> Option<Field> {
        self.kept
            .retain(|(wires, _)| wires.iter().any(|wire| trace.kept.contains(wire)));
        let middle = |wire: &str| {
            let (_, points) = self.routes.iter().find(|(id, _)| id == wire)?;
            let middle = points.get(points.len() / 2)?;
            Some(at_pointer(camera.to_screen(*middle)))
        };
        let mut boxes: Vec<(String, Rect)> = Vec::new();
        // The fields of each kept statement's lines, by statement.
        let mut fields: Vec<(String, Vec<Option<Field>>)> = Vec::new();
        for (wires, anchor) in &self.kept {
            let live: Vec<String> = wires
                .iter()
                .filter(|wire| trace.kept.contains(*wire))
                .cloned()
                .collect();
            let Some(text) = trace.statement(&live) else {
                continue;
            };
            let near = match anchor {
                Anchor::Board(at) => Some(at_pointer(camera.to_screen(*at))),
                Anchor::Chip => tags
                    .iter()
                    .zip(laid)
                    .find(|(tag, _)| tag.wires.iter().any(|wire| live.contains(wire)))
                    .map(|(_, laid)| laid.rect),
            };
            if let Some(near) = near.or_else(|| live.first().and_then(|wire| middle(wire))) {
                fields.push((text.clone(), trace.fields(&live)));
                boxes.push((text, near));
            }
        }
        // A wire selected some other way states itself at its middle.
        let mut loose: Vec<&String> = trace
            .kept
            .iter()
            .filter(|wire| !self.kept.iter().any(|(kept, _)| kept.contains(*wire)))
            .collect();
        loose.sort();
        for wire in loose {
            if let (Some(text), Some(near)) =
                (trace.statement(std::slice::from_ref(wire)), middle(wire))
            {
                fields.push((text.clone(), trace.fields(std::slice::from_ref(wire))));
                boxes.push((text, near));
            }
        }
        for (text, near) in listed.into_iter().chain(pointed) {
            if !boxes.iter().any(|(kept, _)| kept == text) {
                boxes.push((text.clone(), *near));
            }
        }
        #[cfg(test)]
        {
            self.boxes.clear();
            self.board = Some(board);
        }
        let mut clicked = None;
        for (text, near) in boxes {
            let lines = fields
                .iter()
                .find(|(kept, _)| *kept == text)
                .map(|(_, lines)| lines.clone())
                .filter(|lines| editable && lines.iter().any(Option::is_some))
                .unwrap_or_default();
            let within = clear_of(painter, near, &text, board, taken, !lines.is_empty());
            let _shown = if lines.is_empty() {
                readers::paint_statement(painter, near, &text, within)
            } else {
                let actions: Vec<bool> = lines.iter().map(Option::is_some).collect();
                let (shown, buttons) =
                    readers::paint_statement_actions(painter, near, &text, &actions, within);
                for (field, button) in lines.into_iter().zip(buttons) {
                    let (Some(field), Some(button)) = (field, button) else {
                        continue;
                    };
                    let name = format!("Change source of {}", field.name);
                    let response = ui
                        .interact(
                            button.intersect(board),
                            ui.id().with(("change-source", &field.name)),
                            Sense::click(),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    response.widget_info(move || {
                        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name)
                    });
                    if response.clicked() {
                        clicked = Some(field);
                    }
                }
                shown
            };
            #[cfg(test)]
            self.boxes.push((text, _shown));
        }
        clicked
    }
}

/// Where a statement at `near` stands as `readers::paint_statement` places
/// it inside `within`, wider by *Change source* with `actions` (LAY-42).
fn statement_box(painter: &Painter, near: Rect, text: &str, within: Rect, actions: bool) -> Rect {
    let size = readers::statement_size(painter, text, actions);
    let mut at = pos2(near.left(), near.top() - size.y - 4.0);
    if at.y < within.top() {
        at.y = near.bottom() + 4.0;
    }
    at.x = at.x.min(within.right() - size.x).max(within.left());
    at.y = at.y.min(within.bottom() - size.y).max(within.top());
    Rect::from_min_size(at, size)
}

/// The part of `board` a statement at `near` is placed in so that it keeps
/// clear of `taken`: where it would stand under one of them, it stands
/// below it, or above it where there is no room below (LAY-15).
fn clear_of(
    painter: &Painter,
    near: Rect,
    text: &str,
    board: Rect,
    taken: &[Rect],
    actions: bool,
) -> Rect {
    let mut within = board;
    for _ in 0..8 {
        let shown = statement_box(painter, near, text, within, actions);
        let Some(over) = taken.iter().find(|rect| rect.expand(2.0).intersects(shown)) else {
            break;
        };
        let below = Rect::from_min_max(pos2(within.left(), over.bottom() + 4.0), within.max);
        let above = Rect::from_min_max(within.min, pos2(within.right(), over.top() - 4.0));
        within = if below.height() >= shown.height() {
            below
        } else if above.height() >= shown.height() {
            above
        } else {
            break;
        };
    }
    within
}

impl Trace {
    /// What `board` traces: the line `pointed` at, every data wire into the
    /// input `row` pointed at, and the data wires of `selected`.
    pub fn of(
        board: &Board<'_>,
        pointed: Option<&str>,
        row: Option<(&str, &str)>,
        selected: &[String],
    ) -> Self {
        let data: HashSet<String> = board
            .wires
            .iter()
            .filter(|wire| !wire.control)
            .map(|wire| wire.id.clone())
            .collect();
        let mut traced: Vec<String> = pointed.map(str::to_owned).into_iter().collect();
        if let Some((reader, input)) = row {
            traced.extend(
                board
                    .wires
                    .iter()
                    .filter(|wire| {
                        !wire.control
                            && wire.input == input
                            && board
                                .cards
                                .get(wire.target)
                                .is_some_and(|card| card.id == reader)
                    })
                    .map(|wire| wire.id.clone()),
            );
        }
        let kept: HashSet<String> = selected
            .iter()
            .filter(|wire| data.contains(*wire))
            .cloned()
            .collect();
        let mut wires = HashMap::new();
        for id in traced.iter().chain(&kept) {
            let Some(wire) = board.wires.iter().find(|wire| &wire.id == id) else {
                continue;
            };
            let Some(target) = board.cards.get(wire.target) else {
                continue;
            };
            let (source, node) = match &wire.start {
                Start::Output { card, .. } => {
                    let Some(card) = board.cards.get(*card) else {
                        continue;
                    };
                    (card.id.clone(), Some(card.id.as_str()))
                }
                Start::Chip(index) => {
                    let Some(chip) = board.chips.get(*index) else {
                        continue;
                    };
                    (chip.id().to_owned(), chip.source_node())
                }
            };
            let statement = node.map_or_else(String::new, |node| {
                readers::texts(board.graph, board.project, wire, &target.id, node, None).1
            });
            // A step's field the Inspector reads from another step (LAY-42).
            let field = board
                .graph
                .nodes
                .iter()
                .find(|node| node.id == target.id)
                .filter(|reader| {
                    node.is_some()
                        && !statement.is_empty()
                        && crate::inspector::changes_source_at(
                            reader,
                            board.graph,
                            board.project,
                            &wire.input,
                            &wire.operand_path,
                        )
                })
                .and_then(|_| {
                    Some(Field {
                        reader: target.id.clone(),
                        input: wire.input.clone(),
                        path: wire.operand_path.clone(),
                        name: statement.rsplit(" → ").next()?.to_owned(),
                    })
                });
            wires.insert(
                id.clone(),
                Traced {
                    source,
                    target: target.id.clone(),
                    statement,
                    field,
                },
            );
        }
        let ends = kept
            .iter()
            .filter_map(|id| wires.get(id))
            .flat_map(|wire| [wire.source.clone(), wire.target.clone()])
            .collect();
        Self {
            pointed: traced,
            kept,
            ends,
            wires,
            data,
        }
    }

    /// The wires drawn at full strength: pointed at or kept.
    pub fn lit(&self) -> impl Iterator<Item = &String> {
        self.pointed.iter().chain(&self.kept)
    }

    /// Whether the rest of the board dims: something is kept.
    pub fn dims(&self) -> bool {
        !self.kept.is_empty()
    }

    /// Whether the card `id` dims: something is kept and it is at no end.
    pub fn dims_card(&self, id: &str) -> bool {
        self.dims() && !self.ends.contains(id)
    }

    /// The statements of `wires`, one per line, each once.
    pub fn statement(&self, wires: &[String]) -> Option<String> {
        let mut lines: Vec<&str> = Vec::new();
        for wire in wires {
            if let Some(traced) = self.wires.get(wire)
                && !traced.statement.is_empty()
                && !lines.contains(&traced.statement.as_str())
            {
                lines.push(&traced.statement);
            }
        }
        (!lines.is_empty()).then(|| lines.join("\n"))
    }

    /// By line of the statement of `wires`, the field whose source its
    /// *Change source* opens, if any (LAY-42).
    fn fields(&self, wires: &[String]) -> Vec<Option<Field>> {
        let mut lines: Vec<(&str, Option<Field>)> = Vec::new();
        for wire in wires {
            let Some(traced) = self.wires.get(wire) else {
                continue;
            };
            if traced.statement.is_empty() {
                continue;
            }
            match lines.iter_mut().find(|(line, _)| *line == traced.statement) {
                Some((_, field)) => {
                    if field.is_none() {
                        *field = traced.field.clone();
                    }
                }
                None => lines.push((&traced.statement, traced.field.clone())),
            }
        }
        lines.into_iter().map(|(_, field)| field).collect()
    }

    /// The statement of what is pointed at.
    pub fn pointed_statement(&self) -> Option<String> {
        self.statement(&self.pointed)
    }

    /// The ports at both ends of each traced wire `drawn` holds, on screen,
    /// to ring in the accent.
    pub fn rings<'a>(
        &self,
        drawn: impl IntoIterator<Item = &'a (String, Vec<Pos2>)>,
        camera: Camera,
    ) -> Vec<(String, Pos2, Color32)> {
        let mut rings = Vec::new();
        for (id, points) in drawn {
            let Some(traced) = self.wires.get(id) else {
                continue;
            };
            if let (Some(first), Some(last)) = (points.first(), points.last()) {
                rings.push((
                    traced.source.clone(),
                    camera.to_screen(*first),
                    theme::ACCENT,
                ));
                rings.push((
                    traced.target.clone(),
                    camera.to_screen(*last),
                    theme::ACCENT,
                ));
            }
        }
        rings
    }
}

impl super::Canvas {
    /// The statements the last frame showed in boxes, with each box on
    /// screen.
    #[cfg(test)]
    pub(crate) fn debug_statements(&self) -> Vec<(String, Rect)> {
        self.trace.boxes.clone()
    }

    /// The board's area on screen in the last frame.
    #[cfg(test)]
    pub(crate) fn debug_board(&self) -> Rect {
        self.trace.board.unwrap_or(Rect::NOTHING)
    }
}
